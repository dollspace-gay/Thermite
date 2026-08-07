# Frozen primitive registry and exact boundary refinement

<!--
tier: 3-component
status: partial
audited-content-sha256: 4704d62c89c5a738c858a9536c9b4e28d60000d510c6ad697a765a594360b4ce (re-pinned 2026-08-07 after the synthetic platform fixtures joined this doc's routed governed set)
decision: consumer-owned registry entries close reachable Thermite boundaries through non-exempt same-crate or separately compiled/imported direct-Verus calls
governs:
  - thermite-lower/src/lower.rs
  - thermite-lower/src/lib.rs
  - thermite-lower/tests/l3_library.rs
  - forge/src/cli.rs
  - forge/src/verified_build.rs
  - forge/src/verified_build/composition.rs
  - forge/src/verified_build/primitive_registry.rs
  - forge/tests/verified_composition.rs
  - conformance/verified-composition/frozen_primitive.th
  - conformance/verified-composition/frozen_primitive_shell.rs
  - conformance/verified-composition/frozen_primitive_registry.json
  - conformance/verified-composition/synthetic_platform.th
  - conformance/verified-composition/synthetic_platform_shell.rs
  - conformance/verified-composition/synthetic_platform_registry.json
  - conformance/verified-composition/separate_primitive_impl.rs
  - conformance/verified-composition/separate_primitive_shell.rs
  - conformance/verified-composition/separate_primitive_registry.json
  - conformance/verified-composition/machine_atomic.th
  - conformance/verified-composition/machine_atomic_impl.rs
  - conformance/verified-composition/machine_atomic_shell.rs
  - conformance/verified-composition/machine_atomic_registry.json
  - conformance/verified-composition/machine_atomic_consumer.rs
  - stdlib/kernel-primitives/platform.thpkg.json
  - stdlib/kernel-primitives/platform/api.th
  - forge/tests/platform_primitives.rs
extends:
  - .design/build/kernel-primitives.md
  - .design/build/l3-rich-composition.md
  - .design/lower/boundary-composition.md
thesis-refs:
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §8
  - thermite-design.md §9
-->

## Scope

Forge accepts an optional consumer-owned frozen primitive registry on a strict
L3 composition build:

```text
forge build package.thpkg.json --level l3 \
  --compose-export kernel_entry \
  --compose-shell platform_shell.rs \
  --primitive-registry platform.registry.json \
  --target kernel
```

The registry is a closure and refinement input, not an operation catalog.
Thermite contains no built-in x86, ARM, RISC-V, firmware, scheduler, allocator,
or device profile. A consumer declares only the platform operations its source
contains. Forge resolves those declarations against the source-reachable
boundary closure selected by the link and composition roots.

Thermite now ships 74 architecture-neutral frozen declarations in
`platform.thpkg.json`, plus the separate 50-operation atomic surface and three
wait operations. Those declarations define reusable semantic contracts; they
are not registry entries and contain no implementation. A consumer registry
still closes only its reachable subset, with consumer-owned target and object
evidence.

Registry v1 supports exact same-crate Rust-ABI functions authored in a
direct-Verus shell with `sequential` concurrency semantics. Registry v2 adds a
`separate_verus_crate` linkage for the safe sequential subset. Forge compiles
that authored source in its own strict Verus crate, exports the checked proof
interface, links the exact emitted rlib into the generated caller, and records
the hashes of every emitted object member. Canonical non-empty target-feature
sets are bound into both proof/codegen invocations and their replay.

Registry v3 adds the first machine-aware vertical slice: one canonical
`PAtomicU64` create-and-SeqCst-load adapter. The adapter itself is an ordinary
bodyful direct-Verus function proved at L3, while the exact pinned vstd atomic
source, full codegen rlib, adapter source/interface/rlib, and every emitted
adapter object are receipt-bound and replayed. Verus marks its own atomic
implementation `external_body`, so the hardware crossing remains an explicit
L1 residual assumption. A v3 bundle is consequently reported as
`L1/to_machine_boundary`; it is never laundered into an end-to-end L3 artifact.

Foreign ABIs, consumer unsafe Rust, assembly, volatile access, privileged
instructions, persistent sealed-cell ABIs, and general concurrent execution
remain unsupported. Separate-crate byte closure cannot be repurposed to claim
those classes. They continue to fail closed until their exact implementation
and honest residual/refinement evidence are represented by a later v3
operation or successor schema. Which class a declaration falls into is decided
from its source effect row by "Source-derived minimum machine class" below.

## Schema v1

The top-level schema is `thermite.frozen-primitive-registry.v1` and contains an
exact target triple, a strictly sorted unique list of canonical bare target
features, and entries strictly sorted by semantic name. Unknown JSON fields are
errors. Forge renders a non-empty set exactly once as the canonical
`-C target-feature=+f1,+f2` argument; the argument is part of the artifact plan,
whole-crate result, receipt root, validation, and replay. Registry values cannot
carry their own `+`/`-` prefixes or inject an additional compiler option. Every
requested name must also occur in the pinned codegen rustc's
`--print target-features` inventory. That sorted inventory is itself recorded
in `CodegenRustcEvidence`, included in the path-independent toolchain identity,
validated from the receipt, and reconstructed during replay; an unknown feature
cannot degrade to a compiler warning and silently disappear.

Each entry declares:

- a canonical versioned semantic name and whether it must be reachable;
- the exact Thermite function, `#[boundary]` target, normalized signature,
  contract digest, effect digest, and effect row;
- parameter and result ownership (`by_value`, shared/exclusive borrow,
  sealed consume, or sealed mint) derived independently from the AST;
- the shell module, checked function item, exact shell-source digest, Rust ABI,
  same-crate symbol, and power-of-two alignment;
- `thermite_contract` as the v1 model and
  `same_crate_verus_checked_wrapper` as the refinement mechanism;
- the mandatory `contract_refinement`, `exact_implementation_call`, and
  `whole_crate_no_cheating` obligations;
- the `sequential` concurrency class, an empty memory-ordering list, and failure
  behavior. The parser recognizes the future `atomic`, `volatile`, and
  `privileged` vocabulary only to return a precise unsupported-assurance error;
  registry v1 never accepts those classes into a plan.

The registry cannot weaken or replace source facts. Forge independently derives
the function signature, contract/effects digests, effects, and ownership, then
rejects any mismatch. It independently inventories the shell and rejects a
missing, private, declaration-only, digest-drifted, duplicate, or non-function
implementation.

## Schema v2: separate safe-Rust crate closure

`thermite.frozen-primitive-registry.v2` retains every v1 declaration and makes
`implementation.linkage` mandatory. `same_crate` has the v1 meaning.
`separate_verus_crate` selects a whole source module as a separate crate; a
module cannot mix linkage modes. Its exact symbol remains
`crate_name::checked_item`, so the generated Thermite wrapper calls the imported
crate directly rather than a parallel same-crate implementation.

For each separate crate, the frozen plan binds:

- the authored module path, length, digest, public-item inventory, and generated
  complete crate-source digest;
- the exact target and feature set;
- mandatory `contract_refinement`, `exact_implementation_call`,
  `exported_verus_interface`, `imported_call_refinement`,
  `separate_source_identity`, `separate_object_identity`, and
  `whole_crate_no_cheating` obligations.

Forge first runs `verus --no-cheating --compile --export` on the generated
crate. The kernel form is `#![no_std]` and uses only Verus builtins, so the
emitted implementation has no hosted or `vstd` runtime dependency. Forge then
imports the emitted `.vir` interface and passes the exact emitted rlib as the
Rust dependency of the canonical Thermite caller proof/codegen. The caller
source contains `extern crate <name>` and its generated wrapper calls
`<name>::<item>`.

The receipt independently binds the authored and generated sources, separate
Verus result, exported interface, rlib, and every `.o` archive member's name,
length, and SHA-256. Forge parses the rlib archive itself; an external `ar` tool
does not define the inventory. Validation rejects a changed source, interface,
rlib, archive member, proof result, plan, or receipt.

Verus's serialized `.vir` includes nondeterministic internal metadata, so replay
does not falsely require byte reproduction of a fresh export. It re-verifies the
same generated source, requires byte-identical rlib and object members, imports
the freshly checked interface into the caller, and requires the final caller
rlib to reproduce exactly. The original interface bytes remain individually
bound and tamper-evident in the receipt.

## Schema v3: machine-aware atomic pilot

`thermite.frozen-primitive-registry.v3` retains the safe v1/v2 linkages and adds
`separate_verus_machine_crate`. The initial admitted operation is deliberately
narrow and canonical: `p_atomic_u64_seq_cst_roundtrip` accepts one `u64`, creates
a `vstd::atomic::PAtomicU64`, and returns its SeqCst load under the matching
tracked permission. Its authored adapter bytes must match the canonical checked
body exactly. Returning the input directly, moving the atomic call into dead
code, changing the type or ordering, or substituting a digest-updated safe model
is rejected during planning.

The entry must declare:

- model `pinned_vstd_atomic_permission`, refinement
  `separate_crate_verus_machine_import`, concurrency `atomic`, and exactly the
  `seq_cst` ordering;
- ten sorted discharged obligations covering the Thermite contract, exact
  adapter call, permission and ordering refinement, exported/imported Verus
  interface, source/object identity, pinned model, and both no-cheating layers;
- the three sorted residual assumptions `hardware_atomic_semantics`,
  `pinned_vstd_external_body`, and `rust_core_atomic_codegen`.

Forge compiles the adapter against the full pinned no-std vstd rlib rather than
the proof-only slice/array link shim. The final Thermite caller uses that same
crate identity, preventing a different metadata crate from satisfying the
machine adapter dependency. The receipt carries the full vstd rlib and the
exact `vstd/atomic.rs` source in addition to the aggregate vstd source-tree and
`vstd.vir` identities already present in toolchain evidence. Replay reconstructs
both Verus invocations and requires the same adapter rlib and object members.

This is a proof of the bodyful adapter relative to the pinned Verus atomic
permission model, plus exact evidence for what machine code was emitted. It is
not a proof that Verus's `external_body`, Rust/LLVM atomic codegen, or the target
hardware memory model is correct. Those three facts are counted separately as
residual assumptions, not discharged obligations. The boundary member remains
`L1-residual-machine-assumption`; a distinct checked-wrapper member reports
`L3-relative-to-pinned-machine-model`, and ordinary Thermite callers retain
their own L3 certificate.

## Source-derived minimum machine class

`entry.concurrency` is a string the registry author writes. The function's
`fx platform(...)` row is a source fact that Forge reconstructs from the AST and
already compares against `entry.effects` in `fn plan_from_bytes` in
`primitive_registry.rs`. Machine class follows that source fact. A declaration
may agree with the source-derived class or raise it; it never lowers it.

`.design/build/kernel-primitives.md` §"Sealed authority and platform effects"
freezes the effect family at twelve atoms, and each one carries a minimum
machine class in the registry's existing `sequential`, `volatile`, `atomic`,
`privileged` vocabulary. The map below is total over that family. Each class is
a floor: it admits nothing on its own, and no self-declaration goes beneath it.

| Effect atom | Minimum class | Basis |
|---|---|---|
| `platform(boot)` | `privileged` | `platform-primitives.md` §"Frozen families" gives boot as "normalize a handoff and transfer terminal control". A firmware handoff ABI and a terminal control transfer are the entry/return assembly that this document's Scope lists as unsupported. |
| `platform(memory)` | `privileged` | The same table's memory/provenance row spans "capability-backed addresses, raw/volatile access, page-table words, activation, local invalidation, cache maintenance". Page-table words and address-space activation are mode-restricted system state. |
| `platform(mmio)` | `volatile` | `kernel-primitives.md` §"Irreducible platform-operation families" gives MMIO as "aligned volatile reads/writes by width and device barriers". Device rights arrive as a sealed `DeviceRegion`, which the registry already models as ownership. |
| `platform(pio)` | `volatile` | The same row covers PIO. `kernel-primitives.md` §"Explicit non-goals" forbids a concrete architecture profile, so the class follows the design's own volatile characterization rather than a per-architecture instruction privilege. |
| `platform(irq)` | `privileged` | `platform-primitives.md` §"Frozen families" gives "state-token acquisition, disable/restore, routing, mask/unmask, EOI" and "checked context entry and terminal return". Interrupt state and trap entry/return are processor-mode state. |
| `platform(cpu)` | `privileged` | The same table gives "CPU identity/features, control/register access, per-CPU base". Control and model-specific register access is mode-restricted. |
| `platform(atomic)` | `atomic` | `sealed-atomics.md` §"Source surface": "exactly 50 bodyless L1 declarations with `fx platform(atomic)`". Registry v3's pilot declares concurrency `atomic` for the same family. |
| `platform(smp)` | `atomic` | `platform-primitives.md` §"Frozen families" gives "AP transport, IPI transport, online snapshot and acknowledgement". Another CPU observes and acknowledges, which needs the inter-agent ordering relation of `sealed-atomics.md` §"Finite concurrency model". |
| `platform(dma)` | `atomic` | The same table gives "mapping, unmapping, and ownership-direction synchronization". A device agent observes the mapped region while the CPU runs, and the sync operations are that transfer's ordering. |
| `platform(clock)` | `volatile` | The same table gives "monotonic observation and deadline arm/cancel". A deadline arm programs a timer that raises an interrupt later, and a monotonic read observes a counter the program does not compute. A safe body that satisfies the declared identity and deadline relation reaches neither. |
| `platform(entropy)` | `volatile` | The same table gives "capability-backed fill". The operation draws from a machine entropy source and writes through a `RawAddress`. A constant satisfies the declared permit, generation, and length relation without doing either. |
| `platform(power)` | `privileged` | The same table gives "terminal reboot and poweroff", and both declarations carry `diverge` in `fn power_reboot_terminal` and `fn power_off_terminal` in `platform/api.th`. |

No atom maps to `sequential`. That follows from what the twelve atoms are.
`platform-primitives.md` §Scope describes every row carrying one of them as an
operation whose "meaning ultimately depends on firmware ABI, pointer provenance,
volatile or privileged machine behavior, concurrent hardware, terminal transfer,
or a consumer-owned platform resource", and §"Consumer refinement rule" requires
that for each reachable machine row the receipt bind "an operation-specific
machine or concurrency model" and "validation/replay of both proof layers
without substituting a safe reference implementation". A safe v1/v2 linkage
supplies neither. The absence of a `sequential` row is therefore a result of
reading the family, not an omission from the map.

`sequential` stays reachable through the empty maximum: a `#[boundary]` whose
effect row carries no platform atom. `fn foreign_identity` in
`conformance/verified-build/boundary.th` is that shape, declared `fx pure`.
Those are the "sequential safe-Rust operations" the Scope above names, and they
are the domain registry v1 and v2 keep.

The map governs registry entries. An ordinary bodyful Thermite function may
carry a platform atom in its own row without being a door: `fn write_byte` in
`conformance/kernel_primitives.th` declares `fx platform(memory)` over a
verified in-language body, and it is neither a boundary nor a registry entry.

The class is per atom because the effect row is the only source-derived signal a
registry entry carries. Where one atom spans doors of different machine
character, the atom takes the strongest class its family reaches:
`platform(memory)` spans a bounded byte copy and an address-space activation,
and `platform(clock)` spans a counter observation and a deadline arm that
programs a timer. A finer split needs a finer effect vocabulary, and the family
is frozen at these twelve atoms by `kernel-primitives.md` §"Sealed authority and
platform effects"; widening it is a design amendment, not a registry-local
decision.

The four classes are ordered:

```text
sequential < volatile < atomic < privileged
```

`volatile` adds a single-agent visibility and non-elision requirement to a
sequential model. `atomic` adds the inter-agent relation that
`sealed-atomics.md` §"Finite concurrency model" names (modification order,
reads-from, release sequences, synchronizes-with, happens-before, and SC
precedence), which subsumes that visibility requirement. `privileged` adds
processor-mode and system state, which no shipped schema models at all. The
order exists so that a whole effect row has a maximum; it grants no admission.
Registry v3 admits by exact canonical operation, so a `volatile` door gains
nothing from sitting below the pilot's `atomic`.

### The gate

For a registry entry `e` over Thermite function `f`:

```text
source_class(f)     = max { class(a) | platform(a) occurs in f's effect row }
                      sequential, when f's row carries no platform atom
declared_class(e)   = e.concurrency
effective_class(e)  = max(source_class(f), declared_class(e))
```

1. A safe linkage, `same_crate` in v1 and `separate_verus_crate` in v2, admits
   `e` only when `effective_class(e)` is `sequential`. Any greater class rejects
   before planning, and the diagnostic names `f`, the atom that produced the
   class, and the class. This subsumes the existing refusal, which reads
   `entry.concurrency` alone and therefore misses a door whose source row is
   `platform(atomic)` under a `sequential` declaration.
2. A self-declaration raises the effective class and never lowers it.
   `"concurrency": "sequential"` over a door whose row contains
   `platform(atomic)` is an entry with effective class `atomic`, and a safe
   linkage rejects it.
3. `separate_verus_machine_crate` continues to admit by exact canonical
   operation. The effective class is computed the same way and enters the plan
   next to the declared concurrency; the machine evidence the entry must bind is
   the evidence for the effective class.
4. The resolved plan records the effective class alongside the declared
   concurrency, so validation and replay reconstruct the same decision from the
   same bytes rather than recomputing it from prose.

### Doors no schema models

Registry v1 and v2 model `sequential`. Registry v3 models one canonical atomic
operation. Every `volatile` and `privileged` door, and every `atomic` door other
than that pilot, sits outside all three. Such a door has two admissible outcomes:

1. the registry rejects before planning and the build publishes nothing, which
   is the Acceptance requirement that otherwise well-formed machine classes
   reject through the safe linkages; or
2. the door publishes under the machine cap of "Exact checked-wrapper
   refinement": its boundary member reports `L1-residual-machine-assumption`,
   the composition binding records the effective class and counts at least one
   residual machine assumption, and the artifact aggregate is
   `L1/to_machine_boundary`.

A reachable door that no registry entry covers keeps the existing strict
refusal. The obligation stays visible in the artifact in every case: a
registered machine door never reaches `L3-direct-refinement`, and an artifact
whose reachable registered doors include one above `sequential` never reports
`assurance = L3` with `scope = end_to_end`.

### Tracked artifacts this map decides

The map settles the standing of every tracked registry fixture and of every
frozen declaration in the two machine packages.

| Artifact | Atom | Effective class | Standing |
|---|---|---|---|
| `conformance/verified-composition/frozen_primitive_registry.json` over `frozen_primitive.th` | `clock` | `volatile` | Invalid. It binds `same_crate` linkage to a `fx platform(clock)` door under a `"sequential"` declaration and publishes one `L3-direct-refinement` boundary end to end. |
| `conformance/verified-composition/separate_primitive_registry.json` | `clock` | `volatile` | Invalid on the same ground through `separate_verus_crate`. |
| `conformance/verified-composition/machine_atomic_registry.json` | `atomic` | `atomic` | Valid. It uses `separate_verus_machine_crate`, declares concurrency `atomic`, and reports the L1 machine cap. |
| the inline `"effects": ["platform(clock)"]` registry in the `mod tests` fixtures of `primitive_registry.rs` | `clock` | `volatile` | Invalid on the same ground as the fixture it mirrors. |
| the inline `"effects": ["platform(atomic)"]` machine registry in the same fixtures | `atomic` | `atomic` | Valid under the machine linkage. |
| all 74 declarations in `stdlib/kernel-primitives/platform/api.th` | the eleven non-atomic atoms | `volatile` or `privileged` | No safe linkage closes any of them. |
| all 50 declarations in `stdlib/kernel-primitives/src/machine.th` | `atomic` | `atomic` | No safe linkage closes any of them; v3 admits only its one canonical operation. |
| `fn foreign_identity` in `conformance/verified-build/boundary.th` | none | `sequential` | A safe linkage closes it. This is the shape the v1/v2 domain has. |
| `conformance/verified-composition/synthetic_platform_registry.json` over `synthetic_platform.th` | none | `sequential` | Valid. Two `fx pure` doors carry the sealed ownership transition through `same_crate` linkage. |

The two clock fixtures are this document's synthetic identity primitive, and an
identity body is the right shape for exercising the machinery. The effect row is
what puts them on the wrong side of the rule: `fx platform(clock)` announces a
machine row, and `kernel-primitives.md` §"Acceptance matrix" asks the synthetic
platform to "exercise the registry/refinement machinery without booting or
implementing a kernel", which a boundary carrying no platform atom already does.
They are a second instance of the defect the atomic divergence tests pin, found
through a different atom, and the Acceptance list below states the synthetic
primitive's effect row accordingly. A gate that lands against the fixtures as
they stand turns the two safe-linkage acceptance tests red, and that outcome is
the rule working rather than a regression it caused.

## The synthetic test platform

`.design/build/kernel-primitives.md` §"Acceptance matrix" asks for a synthetic
test platform "whose bodies are tiny direct-Verus adapters", exercising the
registry and refinement machinery "without booting or implementing a kernel".
`conformance/verified-composition/synthetic_platform.th` is that platform.

The scalar identity fixtures above close the `by_value` corner of Schema v1's
ownership vocabulary. The synthetic platform closes the sealed corner. It
declares two sealed types, `SynRegion` and `SynAddress`, and two `#[boundary]`
doors that mirror the declaration shape of `fn raw_address_from_region` and
`fn raw_address_advance` in `stdlib/kernel-primitives/platform/api.th`:

| Door | Parameter ownership | Result ownership |
|---|---|---|
| `fn syn_address_from_region(region: &SynRegion, offset: usize) -> SynAddress` | `shared_borrow`, `by_value` | `mint_sealed` |
| `fn syn_address_advance(address: SynAddress, length: usize) -> SynAddress` | `consume_sealed`, `by_value` | `mint_sealed` |

The mirror stops at the effect row. The two platform doors carry
`fx platform(memory)`, whose effective class is `privileged`, so no safe linkage
closes them. The synthetic doors carry `fx pure`, which reaches `sequential`
through the empty maximum of "Source-derived minimum machine class" above, so
registry v1 `same_crate` linkage closes both at `L3-direct-refinement`. The
ownership row and the machine class are independent source facts, and the
synthetic platform is what shows the registry treating them that way.

Both adapters are direct-Verus functions of the same size as the identity
shells: each states the door's `requires`/`ensures` and constructs the minted
`SynAddress` from the fields the contract names.
`fn syn_platform_observation` composes them, so the frozen closure reaches both
doors from one composition root and the artifact publishes `assurance = L3` with
`scope = end_to_end` and zero residual machine assumptions.

The `exclusive_borrow` parameter arm has no fixture. None of the 74 tracked
platform declarations takes a `&mut` parameter, so there is no declaration for a
fixture to mirror; a `&mut` door would be an invented shape rather than a
synthetic stand-in for a real one. `result_ownership` rejects a borrowed return
type outright, so its remaining arm is a refusal rather than a composable
transition.

## Reachability closure

Forge computes the union closure of every selected link and composition root.
Every reachable `#[boundary]` needs exactly one registry entry. An entry marked
`required` must be reachable. Unknown Thermite functions, non-boundary
functions, duplicate semantic names, duplicate Thermite functions, duplicate
boundary targets, and duplicate implementation functions are errors.

Optional unreachable entries may describe boundary declarations present in the
same canonical package, but they are not lowered, counted, or claimed as
discharged. A supplied registry with no reachable boundary is rejected so an
irrelevant registry cannot create an assurance label.

## Exact checked-wrapper refinement

Ordinary `forge check` and unregistered composition preserve the established
`external_body` boundary semantics and `to_boundary` scope. Registry composition
uses a different, stricter lowering for each resolved boundary:

```text
Thermite caller
  -> generated boundary function with the exact Thermite req/ens signature
  -> shell_module::implementation(original parameters)
```

The generated boundary function has a real body. It carries no
`external_body`, `assume`, `admit`, `unsafe`, or `unimplemented` exemption. For
`same_crate`, the exact shell bytes are inserted into the same canonical
`verus!` crate, and one `verus --no-cheating --compile` invocation must prove
the wrapper call and compile those same bytes. For `separate_verus_crate`, the
first strict invocation verifies and emits the dependency, and the second
strict invocation imports that checked interface and proves the generated
wrapper's exact cross-crate call. Registry v3 uses the second shape but compiles
the canonical adapter against its pinned machine model. A body returning the
wrong value therefore fails before publication in every mode.

The ordinary per-function certificate for the source boundary remains an honest
L1 boundary declaration. The composition assurance aggregate upgrades a safe
member to `L3-direct-refinement` only after registry closure and the
whole-crate checked-wrapper proof succeed. Callers whose local certificates are
`L3/to_boundary` become end-to-end members of the composed artifact because the
named crossing is closed by that exact refinement. Unregistered crossings never
receive this completion.

A machine member is intentionally different. Its checked adapter is L3 relative
to the pinned model, but the bodyless Thermite boundary remains L1 and caps the
complete artifact at `L1/to_machine_boundary`. This split is encoded in the
receipt aggregate rather than left to prose.

Executable/body translation validation may initially report that a dependency
has no in-language body. Forge completes only that exact refusal when the named
dependency is in the validated registry. It replaces the coarse skipped row by
the expected statement rows and records that the canonical boundary call is
covered by the bound wrapper and no-cheating whole-crate proof. Other skipped,
unsupported, divergent, missing, or unverifiable rows remain fatal.

## Receipt and replay

The composition plan records the complete resolved registry, including which
entries are reachable. The bundle stores the exact input at
`evidence/frozen-primitive-registry.json`. The ordinary bound-file inventory,
artifact plan digest, and receipt root cover those bytes. The composition
binding additionally records the registry digest, reachable primitive count,
discharged refinement-obligation count, and residual-machine-assumption count.
For a machine composition the exact pinned atomic model source and full vstd
codegen rlib are ordinary bound files and cannot be replaced independently of
the receipt.

Validation re-parses the strict schema, re-resolves the source closure, rechecks
every declaration/shell/digest/ownership fact, regenerates the bound wrappers
and separate crate sources, and requires the reconstructed plan and combined
source to be identical. Replay re-runs both proof/codegen layers and reproduces
the separate rlib/object set and final rlib digest. Registry byte tampering,
semantic re-authoring, post-plan mutation, shell drift, dependency artifact
drift, or receipt drift rejects.

## Acceptance

- The synthetic identity primitive, whose effect row carries no platform atom,
  builds as a freestanding composition, validates, replays, and reports one
  `L3-direct-refinement` boundary.
- The combined source calls the exact inventoried shell implementation and
  contains no `external_body` or synthetic unimplemented body.
- Signature, contract, effect, target, ABI, ownership, shell-source, proof-list,
  and schema drift fail closed.
- A non-empty canonical target-feature set is present in the plan and in the
  exact whole-crate proof/codegen arguments; validation and replay reconstruct
  the same argument, while duplicate, unsorted, malformed, or post-plan feature
  changes reject. A syntactically valid feature absent from the pinned rustc
  inventory rejects before proof/codegen.
- Registry tampering and a registry change after plan freeze publish nothing.
- A digest-updated shell whose body violates the Thermite contract reaches the
  real whole-crate proof, fails there, and publishes nothing.
- Otherwise well-formed machine classes reject through the safe v1/v2 linkages;
  v3 admits only the exact canonical atomic roundtrip operation.
- An entry whose Thermite function carries `platform(atomic)`, `platform(smp)`,
  or `platform(dma)` rejects under `same_crate` and `separate_verus_crate`
  linkage whatever its `concurrency` string says, and the build publishes
  nothing. The same holds for the volatile atoms `mmio`, `pio`, `clock`, and
  `entropy` and for the privileged atoms `boot`, `memory`, `irq`, `cpu`, and
  `power`.
- The rejection diagnostic names the Thermite function, the effect atom that
  produced the class, and the effective class.
- A `concurrency` string above the source-derived class raises the effective
  class; one below it rejects the entry.
- A safe linkage over a door declaring `fx platform(clock)` or
  `fx platform(entropy)` rejects on the same ground as the other nine atoms.
  The twelve atoms are covered by the volatile, atomic, and privileged groups,
  so no safe linkage closes any platform door.
- A `#[boundary]` whose effect row carries no platform atom, of the shape
  `fn foreign_identity` in `conformance/verified-build/boundary.th` has, keeps
  closing through both safe linkages at `L3-direct-refinement`.
- The synthetic test platform builds, replays, and publishes both of its doors
  at `L3-direct-refinement` under `assurance = L3` / `scope = end_to_end` with
  six discharged refinement obligations and zero residual machine assumptions.
- Its plan carries the ownership spellings `shared_borrow`, `consume_sealed`,
  `mint_sealed`, and `by_value`, each derived from the AST and matching the
  ownership row of the platform declaration the door mirrors.
- Its contract, effect, and shell-source digests, its normalized signature, and
  both halves of its ownership row fail closed under drift, and a digest-updated
  adapter that drops the advance fails at the whole-crate proof.
- No artifact reports `assurance = L3` with `scope = end_to_end` while a
  reachable registered door's effective class exceeds `sequential`, and such an
  artifact counts at least one residual machine assumption.
- Validation and replay reconstruct the same effective class from the receipt
  bytes.
- A registry-v2 synthetic primitive verifies and compiles as a separate
  freestanding crate, is called by the generated Thermite wrapper, links and
  executes from a downstream consumer, replays both proof layers, inventories
  at least one exact object member, and rejects source/interface/rlib tampering.
- The v3 atomic pilot proves its Thermite caller and checked adapter, executes
  from a downstream consumer, replays both proof layers, binds the exact pinned
  atomic model source/full vstd rlib/adapter object, rejects all three forms of
  tampering, and reports ten discharged obligations plus three visible residual
  assumptions under an L1 machine cap.
- Builds without a registry retain the previous strict policy and continue to
  reject reachable boundaries.

## Remaining work

Registry v1 plus v2 cover same-crate and separately emitted safe sequential
Rust with exact source, proof-interface, rlib, and object identity. Registry v3
adds one honest atomic object/model vertical slice without upgrading its three
literal machine assumptions. The sealed
atomic declaration and finite proof-model package exists, and the generic
platform package now supplies all requested non-atomic declaration families,
but the real sealed atomic ABI is not yet wired through v3. Completion still
requires persistent shared ABI types, the remaining atomic operation/order
matrix, assembly and unsafe/irreducible Rust source/object closure, volatile and
privileged models, and concurrent/liveness composition. None of those broader
claims are made by the pilot.

The source-derived machine-class gate above is enforced in planning. The
resolved plan still records the declared `concurrency` string on its own, so the
effective class is reconstructed by re-deriving it from the receipt-bound
registry and source bytes rather than read back from a plan field. Recording it
next to the declared concurrency, which gate rule 4 asks for, is a
`PlannedPrimitiveEntryV1` schema change in `verified_build.rs` and remains open.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-KPRIM-11 (source-derived machine class) | SHIPPED | `fn source_machine_class` in `primitive_registry.rs` maps the twelve frozen atoms through `fn MachineClass::of_platform_atom` and returns the row maximum with the atom that produced it; `fn plan_from_bytes` takes `let effective_class = source_class.max(declared_class)` and rejects a `same_crate` or `separate_verus_crate` entry whose class is above `sequential`, naming the function, the atom, and the class. Consumer: the composition planner reaches it through `fn load_from_evidence`, so `forge build --primitive-registry` rejects before publication. The two pinned divergence tests `divergence_safe_v1_registry_launders_a_platform_atomic_machine_door` and `divergence_safe_v2_registry_launders_a_platform_atomic_machine_door` in `forge/tests/divergence_registry_v4_matrix.rs` pass, and the v1/v2 acceptance fixtures now close `fn platform_identity` in `conformance/verified-composition/frozen_primitive.th` at `fx pure`, a door whose effect row carries no platform atom. |

| REQ-KPRIM-7 (synthetic test platform) | SHIPPED | `conformance/verified-composition/synthetic_platform.th` declares `fn syn_address_from_region` and `fn syn_address_advance` at `fx pure`; `synthetic_platform_shell.rs` supplies `fn syn_address_from_region_impl` and `fn syn_address_advance_impl` as direct-Verus adapters; `synthetic_platform_registry.json` binds them through `same_crate` linkage with parameter ownership `["shared_borrow","by_value"]` and `["consume_sealed","by_value"]` over `mint_sealed` results. Consumer: `fn syn_platform_observation` composes both doors, so `forge build --primitive-registry` reaches them from one composition root. Verification: `fn synthetic_platform_composes_the_sealed_ownership_transition` in `forge/tests/verified_composition.rs` derives the expected ownership rows from `stdlib/kernel-primitives/platform/api.th`, builds and replays the bundle, requires both members at `L3-direct-refinement` under `L3`/`end_to_end` with six discharged obligations and zero residual assumptions, and runs the six-case drift battery plus the lying-adapter refusal. `fn divergence_no_registry_fixture_exercises_the_sealed_ownership_transition` in `forge/tests/divergence_registry_v4_matrix.rs` passes. |

The registry-wide requirements these rows belong to are REQ-KPRIM-7 (generic
frozen boundary registry) and REQ-KPRIM-9 (exact platform refinement
composition) in `.design/reqs/registry.toml`; both remain `partial` and name the
gate in their remaining scope.
