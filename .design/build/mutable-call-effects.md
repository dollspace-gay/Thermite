# Exact mutable-reference call effects

<!--
tier: 3-component
status: shipped
decision: a statement-position call through one or more pairwise-distinct direct mutable finite-record roots composes by independently interpreting the reachable in-language callee body and copying every nominal field into and out of its exact formal state
governs:
  - thermite-tv/src/exec_encode.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/src/obligation.rs
  - thermite-tv/src/lib.rs
  - thermite-tv/tests/mutable_call_effect_tv.rs
  - forge/src/body_tv.rs
  - forge/src/exec_tv.rs
  - forge/tests/body_tv.rs
  - forge/tests/verified_build.rs
  - conformance/verified-build/mutable_call_effect.th
audited-content-sha256: e6dba30a9c4a1092b4dba2e9b5a63d2850ab5210a98961e6711d47f6833f261d (re-pinned 2026-08-05 after exact dependency and pre-state grounding; mutable-call semantics remain regression-covered)
extends:
  - .design/build/nested-aggregate-lifecycle.md
  - .design/build/owned-aggregate-lifecycle.md
  - .design/verified/exec-stmt-tv.md
  - .design/build/kernel-primitives.md
-->

## Decision

Thermite body translation validation now composes a reachable call with mutable
record inputs when the entire effect frame is derivable from the validated source
closure:

- the callee has an ordinary in-language body, not a boundary or slag body;
- the call is a bare, statement-position function call with exact arity;
- every mutable formal is `&mut Name`, where `Name` belongs to the recursively
  finite structural-record closure;
- every corresponding actual is one direct caller record root with the same
  nominal type; and
- mutable actual roots are pairwise distinct.

Other formals are by-value inputs. A callee mixing shared-reference and mutable
formals is rejected until the source frame can express their exact cross-root
alias relation.

Every unsupported form is rejected before an obligation can be labelled
faithful. The first subset does not infer aliasing, summarize a foreign body, or
replace a call with its authored contract.

## Independent state composition

Forge derives one `MutableCallEffectFrame` from each reachable bodyful callee. It
contains the parsed formal order, exact nominal mutable-record frames, and the
source body. The independent lifecycle semantics applies a call as follows:

1. encode every non-mutable actual against the caller's pre-call state and bind
   it as a read-only formal value;
2. copy every direct field of each caller root into the matching formal root;
3. interpret the callee source body through the independent statement semantics,
   recursively applying any further acyclic mutable-call effects;
4. encode the callee tail under the post-state even though a statement-position
   caller discards its return value; and
5. copy every formal post-state field back to its caller root.

Nested writes remain exact because changing a nested leaf reconstructs its
containing direct field before that field is copied back. Copying the complete
field inventory makes collateral mutations observable. A repeated actual root,
nominal mismatch, missing field, unsupported body form, or recursive effect cycle
returns `Unsupported`; none can silently become a no-op effect.

## Production and expression fidelity

The production obligation includes the ordinary lowered callee and executes the
real generated call. Mutable callees do not receive a pure
`when_used_as_spec` surrogate: their independently reconstructed state appears
only in the body wrapper's postcondition. Consequently, deleting a call,
changing an argument, reordering dependent calls, or weakening the callee's
collateral frame changes a Verus obligation.

Per-expression TV preserves a named-record borrow as `&mut Name`. A field read in
the independent postcondition selects `final(root).field`, while the production
wrapper body executes the ordinary `root.field` read. This is the exact Verus
phase distinction for a wrapper that does not itself mutate the already-reached
program-point state; using the unselected mutable name is an adversarial failure.

## Assurance and acceptance

The focused real-Verus suite proves two dependent mutable calls and a distinct
two-root call, then rejects a dropped second call, wrong argument, missing
collateral callee frame, duplicate exclusive alias, recursive effect cycle, and
missing `final` field selector. Forge's corpus body-TV test derives the effect
from source rather than accepting a hand-authored model, and rejects a callee
mixing shared and mutable references until their alias relation is explicit.

The policy-free `mutable_call_effect.th` fixture builds for the generic
freestanding target at strict L3, replays the receipt, links a downstream Rust
consumer, and executes the two real generated calls. Every reachable member and
every contract, exec, body, and wrapper TV row must be L3/faithful. Bound source
tampering invalidates verification. There is no bodyless declaration in this
increment, so every added application primitive is L3; no L1 exception is used.

This is reusable language and proof machinery. It adds no scheduler, allocator,
boot path, firmware runtime, architecture implementation, or kernel artifact.

## Residual boundary

The frozen subset still excludes mutable slice/array callees, mixed shared and
mutable reference formals, an actual such as `outer.inner` or `slots[i]`,
consumption of a mutable callee's return value in an expression, recursive
mutable effects, mutable enum payloads, calls inside the record-loop theory,
dynamically quantified aggregate frames, and concurrent or atomic machine
effects. Those require separate alias, storage, loop, and machine-refinement
primitives.
