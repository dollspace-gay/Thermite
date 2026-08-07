# Divergent terminal platform operations and the verified build

<!--
tier: 3-component
status: partial
decision: a divergent terminal declaration stays outside the reachable closure of a verified build; assurance over a terminal operation is the proof of the decision that precedes it, and binding the operation itself waits on a registry version that models a privileged door and its progress semantics
governs:
  - stdlib/kernel-primitives/platform/api.th
  - stdlib/kernel-primitives/synchronization/wait.th
  - conformance/verified-build/diverge.th
  - conformance/verified-build/transitive_diverge.th
audited-content-sha256: e4d64d494051fc595ef750f0122c5698130b6acf0d6c8c5ca49b57ba69aab3c0 (initial pin 2026-08-07 for the divergent-terminal composition limit)
extends:
  - .design/build/kernel-primitives.md
  - .design/build/platform-primitives.md
  - .design/build/synchronization-primitives.md
  - .design/build/frozen-primitive-registry.md
  - .design/build/l3-verified-artifact.md
thesis-refs:
  - thermite-design.md §4
  - thermite-design.md §6
  - thermite-design.md §7
  - thermite-design.md §9
-->

## Scope

Eight frozen declarations in the two machine packages carry `diverge` in their
effect row. Seven live in `stdlib/kernel-primitives/platform/api.th` and one in
`stdlib/kernel-primitives/synchronization/wait.th`.

| Declaration | Module | Effect row | Family |
|---|---|---|---|
| `boot_entry_transfer` | `platform/api.th` | `platform(boot), diverge` | boot |
| `runtime_panic_terminal` | `platform/api.th` | `platform(boot), diverge` | runtime |
| `runtime_contract_failure_terminal` | `platform/api.th` | `platform(boot), diverge` | runtime |
| `runtime_allocation_failure_terminal` | `platform/api.th` | `platform(boot), diverge` | runtime |
| `trap_context_return` | `platform/api.th` | `platform(irq), diverge` | trap |
| `power_reboot_terminal` | `platform/api.th` | `platform(power), diverge` | power |
| `power_off_terminal` | `platform/api.th` | `platform(power), diverge` | power |
| `cpu_halt_terminal` | `synchronization/wait.th` | `platform(cpu), diverge` | CPU |

`fn cpu_pause` and `fn wait_block` in `wait.th` are outside this set. Both
return, and both are ordinary machine boundaries governed by
`.design/build/synchronization-primitives.md` §"Waiting surface".

This document states where those eight sit with respect to
`forge build --level l3`. It records the position, the reasoning behind each
refusal, the two consumer routes that carry assurance today, and the one
prerequisite that is missing.

## Position

A divergent operation does not compose into a verified build. `forge build
--level l3` refuses a reachable function whose effect row carries `diverge` and
publishes nothing. `fn strict_source_checks_with_registered_boundaries in
verified_build.rs` performs that refusal before planning, and the refusal has no
carve-out for a registered boundary.

The eight declarations reach L3 or L4 by no route that exists today. Their
`forge check` certificates are L1 boundary rows, which
`.design/build/platform-primitives.md` §Scope already records as the standing of
every machine-facing row in the package. That L1 row is the whole claim available
for a terminal operation now.

The position follows from the design. The four sections below give the
independent grounds.

## Why the refusal stands

### The Termination gate names `fx diverge`

`.design/build/l3-verified-artifact.md` §"Strict gates" enumerates the gates a
verified build runs and the hard failures of each. The Termination row reads:

> | Termination | verified form | `fx diverge`, `decreases *`, no-decreases exemptions |

The same section opens with "Every expected gate has one of two outcomes: `pass`
or build failure. There is no skipped-success state." The refusal in
`verified_build.rs` is that row.

### The partial-correctness cap makes the gate a restatement

`.design/forge/degrade-ladder.md` REQ-9 fixes the honest level of a divergent
function at "L1 = partial correctness", on the ground that such a function is not
total and so cannot claim L3, which §6 of the thesis defines as the contract
holding for all inputs. `fn gate_fn in check.rs` applies that cap before any
prover runs, routing a bodied divergent function to `GateOutcome::DivergeL1`.

The strict-gate table's next row requires "Function certificates | L3 or
stronger, not degraded", with "L0/L1/L2, timeout degradation,
reject/counterexample" as hard failures. A divergent function in the reachable
closure therefore fails the certificate gate on its own. Removing the Termination
row moves the refusal one row down and publishes nothing new.

### Subsumption carries the cap to every caller

The eight declarations take the boundary arm of `fn gate_fn in check.rs` and
certify as L1 boundary rows. The partial-correctness cap lands on their callers.

`.design/lower/effect-subsumption.md` §"The effect lattice (REQ-1)" places
`Diverge` among the nine atoms and defines `subsumes(caller, callee) ⇔
effects(callee) ⊆ effects(caller)`. A caller of a terminal door declares
`diverge` in its own row, and so does that caller's caller, up to the export
root. Every function on the path takes the L1 cap.

`.design/build/kernel-primitives.md` §"Completion rule" holds Thermite-authored
code to a higher floor: "Every Thermite-authored language semantic, model, and
reusable algorithm has an L3-or-L4 assurance floor. ... L2, L1, L0, an unrun
proof, or a skipped translation-validation row is not a completed primitive." The
same section scopes the sub-L3 exception to "a bodyless frozen declaration for an
irreducible machine operation whose implementation is deliberately supplied by a
consuming platform". The exception covers the door and stops there.

Admitting one terminal call would move every function between it and the export
root from the L3 floor to partial correctness.
`conformance/verified-build/transitive_diverge.th` is the pinned three-deep
instance, and the refusal names the path `transitive_diverge_root ->
diverge_middle -> transitive_diverging`.

### The registry refuses these doors on independent grounds

`.design/build/frozen-primitive-registry.md` §"Source-derived minimum machine
class" maps each of the four atoms these declarations carry to the strongest
class: `platform(boot)`, `platform(irq)`, `platform(cpu)`, and `platform(power)`
are all `privileged`. The `power` row states the connection to divergence
directly: "The same table gives 'terminal reboot and poweroff', and both
declarations carry `diverge` in `fn power_reboot_terminal` and `fn
power_off_terminal` in `platform/api.th`."

§"The gate" rule 1 admits a safe `same_crate` or `separate_verus_crate` linkage
only at `sequential`, and rule 3 keeps `separate_verus_machine_crate` admitting
by exact canonical operation, which today is the single `PAtomicU64` SeqCst
roundtrip. §"Tracked artifacts this map decides" applies that to the package
outright, giving all 74 declarations in `platform/api.th` a `volatile` or
`privileged` class and the standing "No safe linkage closes any of them."

A registry entry is therefore unavailable for any of the eight regardless of the
Termination gate. Relaxing the gate alone would leave every one of them refused
by the unregistered-boundary rule in the same function.

## What a consumer does instead

Two routes carry assurance over a terminal operation today. Both keep the
divergent call out of the reachable closure and put the proof where a total
contract holds.

### Route A: the transfer sits outside the artifact

`.design/build/kernel-primitives.md` §"Generic freestanding composition" draws
this line: "`forge build --target kernel` remains a generic `no_std + alloc`
verified library build. It produces an rlib and a receipt; it does not select
firmware, link a kernel, manufacture a disk image, or run QEMU." The section
closes with "Final executable and image construction is deliberately
consumer-owned", and the repository split table in §Scope assigns "firmware
entry, linker script, image packaging, and boot tests" to the consumer column.

The eight declarations are the operations that end a program: firmware entry
transfer, the three runtime failure terminals, trap return, reboot, poweroff, and
halt. A consumer's `no_std` crate links the verified rlib, calls its exported
total functions, and performs the terminal transfer in its own code after the
last exported call returns. The artifact's claim covers the program up to that
call, which is the whole program that has a state to describe.

The two shipped package receipts already have this shape.
`.design/build/platform-primitives.md` §"L3 application-facing basis" records
that "The strict freestanding package receipt exports `platform_width_legal`,
binds the complete authored module even though the machine doors are unreachable,
and replays to the same artifact."
`.design/build/synchronization-primitives.md` §"Assurance and remaining work"
records the same shape for `ticket_lock_can_issue` "while binding all nine
original modules into the receipt". The terminal declarations sit in the bound,
tamper-evident source of both bundles and outside the proved closure of both.

### Route B: the decision to terminate is the exported proof

The assurance a consumer wants over `power_off_terminal` is that the system was
in a state where powering off was correct. That predicate is ordinary total
Thermite, and it proves at L3 as an exported function of the consumer's own
state. The consumer's Rust calls the terminal door on that result.

The door's own contract does not carry that content. Each of the eight declares
`req true` or a shallow legality guard, and an `ens` describing the receipt value
it would name. `.design/build/platform-primitives.md` §Scope states the limit: "A
declaration alone never upgrades a caller to end-to-end L3."

`.design/build/kernel-primitives.md` §"Waiting and synchronization" names the
split this route uses: "The verifier distinguishes safety (proved for every step)
from liveness (proved under named fairness/progress assumptions)." A verified
build proves safety of every step the program takes. The transfer that follows
the last step is a progress fact, and no shipped receipt records a named progress
assumption.

### Route C: the registry version that does not exist

`.design/build/synchronization-primitives.md` §"Waiting surface" names the
missing artifact for these declarations: "A consumer must provide the exact
machine bodies and direct refinements through a registry version capable of
expressing their concurrency and progress semantics."

No shipped version expresses progress semantics, and no shipped version models a
privileged door. `.design/build/frozen-primitive-registry.md` §"Remaining work"
lists both as prerequisites: "Completion still requires persistent shared ABI
types, the remaining atomic operation/order matrix, assembly and
unsafe/irreducible Rust source/object closure, volatile and privileged models,
and concurrent/liveness composition."

This document does not specify that admission rule. Writing it needs the
privileged machine model first, because the shape of a caller's obligation over a
non-returning call depends on what the receipt binds about the transfer. GitHub
issue #133 tracks the route and lists the five questions such a version settles:
the caller obligation shape for a call with no continuation, whether a divergent
call is admitted in tail position only, the emitted Rust and the caller proof
that discharges it, the receipt record and the assurance the artifact may claim
once the privileged class rejects a safe linkage, and which divergent shapes stay
refused whatever the schema admits.

## The fail-closed boundary

Three refusals hold whatever a later registry version admits.

A bodied `fx diverge` function stays out of a verified artifact.
`.design/build/kernel-primitives.md` §"Waiting and synchronization" states the
source-level rule as "An unannotated infinite loop is not accepted as a lock
implementation"; the strict artifact declines an annotated one as well, because
its honest level is L1 and the artifact floor is L3.
`conformance/verified-build/diverge.th` is that shape: a self-recursive `fn
diverging` under `ens result == x` with no decreasing measure.
`examples/editor/editor.th`'s `fn run` is the same class at the language level,
capped at L1 by `fn gate_fn in check.rs` and never built as a verified artifact.

Code after a terminal call stays refused. Anything following an operation that
does not return is unreachable, so a postcondition over it holds for no reachable
state. `thermite-design.md` §7's battery exists to reject a contract that says
nothing about an implementation, and a postcondition over an empty state set is
that contract.

An artifact whose reachable closure contains a divergent declaration does not
report `assurance = L3` with `scope = end_to_end`.
`.design/build/frozen-primitive-registry.md` §"Doors no schema models" states the
general form: "an artifact whose reachable registered doors include one above
`sequential` never reports `assurance = L3` with `scope = end_to_end`". All eight
are `privileged`, so the rule binds every one of them. Today the stronger outcome
applies and nothing publishes at all. This preserves the §9 distinction the
thesis asks the manifest to keep between "verified to the boundary" and
"verified, period."

## Acceptance

- A build whose export closure reaches `fn diverging` in
  `conformance/verified-build/diverge.th` exits non-zero, writes no bundle, and
  names `diverge` in its diagnostic.
- A build whose export closure reaches a divergent function through two
  intermediate callers exits non-zero and names the path
  `transitive_diverge_root -> diverge_middle -> transitive_diverging`.
- Each of the seven `platform/api.th` terminal rows and `cpu_halt_terminal`
  publishes a certificate whose effect list contains `diverge`.
- The platform package builds and replays a strict freestanding receipt while its
  module holds all seven terminal declarations outside the export closure, and
  the synchronization package does the same for `cpu_halt_terminal`.
- No registry entry binds any of the eight, and no artifact reports
  `assurance = L3` with `scope = end_to_end` over a closure containing one.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-DIVTERM-1 (termination gate) | SHIPPED | impl `fn strict_source_checks_with_registered_boundaries in verified_build.rs` returns `"reachable path ... declares fx diverge at ..."` for any reachable function whose row carries `Effect::Diverge`, per `.design/build/l3-verified-artifact.md` §"Strict gates" Termination row. Non-test consumers: `pub fn build_file in verified_build.rs` through `fn strict_source_checks`, `fn assemble in composition.rs` reached from `pub(super) fn build_file in composition.rs`, and `pub fn validate_bundle in verified_build.rs`; the CLI enters at `fn run_build in cli.rs`. Verification: `every_strict_refusal_publishes_nothing in forge/tests/verified_build.rs` runs `conformance/verified-build/diverge.th`, asserts exit code 1, asserts the bundle path does not exist, and asserts the diagnostic contains `diverge`. |
| REQ-DIVTERM-2 (transitive refusal) | SHIPPED | impl `pub fn subsumes in effects.rs` puts `Diverge` in the caller's required row per `.design/lower/effect-subsumption.md` §"The effect lattice (REQ-1)", and `fn closure_path in verified_build.rs` renders the refused chain. Non-test consumers: `pub fn check_effects in effects.rs`, called from `fn prepare_thermite_input in verified_build.rs` and `fn parse_program in build.rs`. Verification: the same acceptance test runs `conformance/verified-build/transitive_diverge.th` and asserts the diagnostic contains `transitive_diverge_root -> diverge_middle -> transitive_diverging`. |
| REQ-DIVTERM-3 (visible terminal vocabulary) | SHIPPED | impl the eight declarations `fn boot_entry_transfer`, `fn runtime_panic_terminal`, `fn runtime_contract_failure_terminal`, `fn runtime_allocation_failure_terminal`, `fn trap_context_return`, `fn power_reboot_terminal`, and `fn power_off_terminal` in `platform/api.th`, plus `fn cpu_halt_terminal` in `wait.th`, each carrying `diverge`. Non-test consumer: `fn gate_fn in check.rs` builds every certificate's effect list through `pub fn effects_of in manifest.rs` and its `fn effect_token`, so the declared effect reaches the published row. Verification: `forge/tests/platform_primitives.rs` asserts `diverge` in the certificate effects of the seven platform rows and `forge/tests/synchronization_primitives.rs` asserts it for `cpu_halt_terminal`. |
| REQ-DIVTERM-4 (consumer routes A and B) | SHIPPED | impl `fn closure_program in verified_build.rs` selects the emitted crate from the reachable closure while the receipt binds the whole authored module, so a package holding terminal declarations builds when the export closure excludes them. Non-test consumers: `pub fn build_file in verified_build.rs` and `fn assemble_from_paths in composition.rs`. Verification: `forge/tests/platform_primitives.rs` builds and replays the platform package exporting `platform_width_legal`, and `forge/tests/synchronization_primitives.rs` builds and replays the synchronization package exporting `ticket_lock_can_issue` while binding all nine modules. |
| REQ-DIVTERM-5 (terminal admission route) | NOT-STARTED | open prereq blocker github:dollspace-gay/Thermite#133. No registry schema models a `privileged` door or records a named progress assumption, so the eight declarations have no entry to bind and no assurance above their L1 boundary certificate. |
