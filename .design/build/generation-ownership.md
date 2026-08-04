# Generation-based ownership primitives

<!--
tier: 3-component
status: partial
decision: Thermite ships an opaque policy-free generation ledger whose verified transitions invalidate stale handles, reject double release, and monotonically narrow rights; complete strict body-TV framing and exact platform refinement remain
governs:
  - stdlib/kernel-primitives/ownership.thpkg.json
  - stdlib/kernel-primitives/ownership/generation.th
  - forge/tests/ownership_primitives.rs
audited-content-sha256: 6bbf66cde8d8444b3146d5462f997d731da4dc24e2f7b7136c1d840fff648c0b (re-pinned 2026-08-04 after complete-certificate L3-floor enforcement)
extends:
  - .design/build/kernel-primitives.md
  - .design/build/frozen-primitive-registry.md
  - .design/build/kernel-target.md
thesis-refs:
  - thermite-design.md §3
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §9
-->

## Decision

Thermite provides a reusable generation discipline, not a kernel capability
ledger. Consumer kernels decide what a slot means, which identities receive
authority, and which rights bits correspond to operations. The library only
provides allocation-free ownership mechanics that those policies can reuse.

The package is `stdlib/kernel-primitives/ownership.thpkg.json`. Its one source
module is `ownership/generation.th`; the manifest, original source, source map,
generated proof source, translation-validation rows, toolchain, and rlib are
bound into strict receipts. There is no Rust or assembly implementation and no
firmware, scheduler, allocator, IPC, DMA, AP, or device policy.

## State and authority

The fixed ledger has 64 slots. Each slot stores:

- an active bit;
- a `u64` generation; and
- a `u64` rights mask.

`GenerationAuthority` is sealed and can only be minted through the frozen
`thermite::ownership::generation_authority` boundary. `GenerationLedger` owns
that non-`Copy` authority together with the three fixed arrays. Moving the
ledger through a transition moves the authority; an L3 consumer cannot use the
same ledger value twice or call `.clone()` because the generated verified type
implements neither `Copy` nor `Clone`. The ledger is also `#[opaque]`: only its
declaring package module may construct the aggregate, and generated external
safe Rust cannot construct or inspect its crate-visible fields.

`GenerationHandle` records only the authority identity, slot, generation, and
rights. It is opaque as defense in depth, so foreign modules obtain handles
through verified acquisition/renewal paths rather than arbitrary literals.
Correctness still does not depend on secrecy or affine handle values: every
operation validates the fields against the unique ledger state, and an accepted
transition either retires the slot or returns a refreshed generation. A stale
copied handle therefore fails the next state check.

## Transition surface

The bodyful Thermite operations are:

- `generation_ledger_init`, which consumes a sealed authority and initializes
  the fixed storage;
- `generation_handle_live`, the exact active/identity/generation/rights check;
- `generation_rights_narrow`, which accepts only a nonzero subset of the
  current mask;
- `generation_acquire_at`, which activates an unused non-exhausted slot and
  increments its generation;
- `generation_renew`, which validates the current handle, rejects escalation,
  increments the generation, and returns the sole refreshed handle; and
- `generation_release`, which validates the handle, clears active and rights,
  and returns no live handle.

Acquisition, renewal, and release return closed result enums. Rejection is an
ordinary explicit value with a stable reason code; it is not a panic, unchecked
boolean, or hidden platform action. Generation exhaustion is fail-closed and
never wraps from `u64::MAX` to zero.

Rights are not interpreted by this package. The only law is monotonicity:
`requested != 0 && (requested & current) == requested`. A kernel may define any
bit allocation above that law without changing Thermite.

## Adversarial properties

The source includes proved lifecycle probes rather than expected runtime
markers:

- acquisition followed by renewal and release succeeds;
- a separately reconstructed handle is rejected on a second release;
- release followed by slot reuse invalidates the prior generation; and
- a request outside the current rights mask is rejected as escalation.

The integration test synthesizes three hostile consumers. One moves the same
authority-bearing ledger into two calls; another calls `.clone()` on the ledger;
and a separate imported package module attempts to construct an opaque handle
literal. The first two cannot certify at L3 and package validation rejects the
third before lowering. These checks establish both value-use and construction
barriers, while the generation probes establish that copied stale handle values
fail after an accepted transition.

## Assurance split

`forge check ownership/generation.th --level l3` currently reports:

- 23 in-language declarations, specifications, transitions, and probes at L3;
- 90/90 executable mutations killed;
- one reachable-capable frozen mint declaration at L1, because the package
  supplies no machine/platform implementation; and
- zero Rust/assembly TPL bodies and zero ordinary Rust kernel-policy lines.

The acceptance gate iterates the complete certificate inventory: every row
other than `generation_authority` must be non-boundary L3. The one exception is
matched by exact name and exact frozen target, so a newly introduced L1/L2
algorithm cannot hide outside a hand-picked positive list.

The package also has a strict freestanding build/replay surface rooted at
`generation_rights_narrow`. It binds the complete package source closure and
requires every recorded translation-validation row to be faithful.

The complete generation-ledger lifecycle is not yet claimed as a strict receipt
export. Strict body TV now independently frames both direct one-level writes
through exclusive finite named-record borrows and typed owned-record local
mutation/pure value-call composition. This package additionally consumes and
returns large aggregate values through nested ADT matches and mutable callee
chains. Those match/result/call-effect forms are still rejected as `skipped`,
rather than silently promoted from the per-item L3 proof. Closing that narrower
gap requires exact ADT result state and mutable callee-effect composition.

Likewise, the frozen authority mint is only a declaration in this package. A
consumer must bind it to an exact implementation and direct refinement. The
safe-Rust registry v1 may be used only if the consumer's implementation truly
fits that assurance class; this package does not claim a machine operation.

## Remaining ownership closure

This increment strengthens REQ-KPRIM-4 but does not complete it. The remaining
work is explicit:

1. extend the shipped owned-record value TV to nested aggregate/enum results,
   ADT matches, and mutable-reference callee effects, then strictly build/replay
   the full generation lifecycle;
2. directly refine the authority-mint implementation supplied by a synthetic
   consumer platform;
3. add a complete affine/linear rule if consumers require uniqueness beyond
   the current sealed-root, move-check, generation, and construction barriers;
4. bind generation ownership into sealed atomic initialization slots; and
5. add concurrent synchronization consumers that rotate generations through
   exact atomic transitions.

Until those close, the accurate claim is “verified generation transition
library with a sealed non-duplicable root and opaque construction,” not
“complete affine ownership system.”

## Auditable metrics

At this increment:

| Metric | Value |
|---|---:|
| Physical Thermite LOC | 534 |
| Nonblank Thermite LOC | 510 |
| Thermite functions | 18 (14 executable, 4 specification) |
| Frozen boundary declarations | 1 |
| Bodyful Rust/assembly primitive implementations | 0 |
| Ordinary Rust kernel-policy/algorithm LOC | 0 |
| Direct-Verus TPL LOC shipped by this package | 0 |
| In-language L3 items | 23 |
| Executable mutants killed | 90/90 |

The Rust integration test is verification harness code, not a runtime
implementation or kernel algorithm.
