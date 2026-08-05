# Exact mutable-reference call effects

<!--
tier: 3-component
status: shipped
decision: a statement-position call or direct typed let-bound result call through structurally disjoint direct or explicitly borrowed projected finite-record and indexed-storage actuals composes by independently interpreting the reachable in-language callee body, shared snapshots, logical intermediate finite-record bindings, leafwise finite-record result, and every complete post-state leaf or sequence
governs:
  - thermite-tv/src/exec_encode.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/src/obligation.rs
  - thermite-tv/src/lib.rs
  - thermite-tv/tests/mutable_call_effect_tv.rs
  - thermite-tv/tests/mutable_indexed_call_effect_tv.rs
  - forge/src/body_tv.rs
  - forge/src/exec_tv.rs
  - forge/tests/body_tv.rs
  - forge/tests/verified_build.rs
  - conformance/verified-build/mutable_call_effect.th
  - conformance/verified-build/mutable_indexed_call_effect.th
  - conformance/verified-build/mixed_indexed_call_effect.th
  - conformance/verified-build/projected_record_call_effect.th
  - conformance/verified-build/projected_indexed_call_effect.th
  - conformance/verified-build/record_after_indexed_call_effect.th
audited-content-sha256: 4a22e849deedee387d14c312916ec385c76a94f0ca87a98c4f148a1827fdead2 (typed intermediate record snapshots after projected indexed state and strict L3 runtime evidence added 2026-08-05)
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
- the call is bare and has exact arity, appearing either in statement position
  or directly as the initializer of one typed `let` binding;
- every mutable formal is either `&mut Name`, where `Name` belongs to the
  recursively finite structural-record closure, `&mut [T]`, or
  `&mut [T; N]`;
- every corresponding record actual is either one direct caller root or an
  explicit `&mut root.field(.field)*` borrow whose entire projection chain lies
  in the finite structural-record closure and ends at the same nominal type;
  every corresponding indexed actual is either one direct already-borrowed root
  or an explicit `&mut root.field(.field)*` borrow whose final field is a slice
  or fixed array with the exact parsed element/capacity type;
- mutable access paths are structurally disjoint across record and indexed
  formals: equal and ancestor/descendant paths overlap, while sibling record
  fields do not;
- every shared formal is `&Name` over the same finite structural closure,
  `&[T]`, or `&[T; N]`; a record actual may be a direct root or an explicit
  `&root.field(.field)*` borrow with the same exact finite nominal type, while an
  indexed actual may likewise be a direct root or an explicit
  `&root.field(.field)*` borrow ending at the exact parsed slice/array type; and
- no shared actual overlaps any mutable actual in the same call. Shared/shared
  aliasing is harmless and remains admitted.

Other formals are by-value inputs. Array-element roots, implicit field
borrowing, and non-finite records remain rejected rather than
receiving an inferred alias, representation conversion, or snapshot relation.

Every unsupported form is rejected before an obligation can be labelled
faithful. The first subset does not infer aliasing, summarize a foreign body, or
replace a call with its authored contract.

## Independent state composition

Forge derives one `MutableCallEffectFrame` from each reachable bodyful callee. It
contains the parsed formal order, exact nominal mutable-record frames, exact
mutable slice/fixed-array pointee types, and the source body. The independent
lifecycle semantics applies a call as follows:

1. encode every non-mutable actual against the caller's pre-call state and bind
   it as a read-only formal value;
2. copy every direct field of each exclusive record root, or project every field
   of an explicitly borrowed nested record from the caller's current enclosing
   value, and copy the complete current finite sequence of each direct or
   projected exclusive slice/array path into the matching
   mutable formal, and snapshot every shared record or complete shared
   slice/array sequence from the caller's current lifecycle state;
3. interpret the callee source body through the independent statement semantics,
   recursively applying any further acyclic mutable-call effects;
4. encode the callee tail under the exact post-state, either discarding it for a
   statement call or binding it to the typed caller local; a final finite-record
   constructor or exact access path is related field-by-field, so an array leaf
   with a current sequence overlay is compared through its complete logical view;
   and
5. copy every formal post-state field or complete sequence back to its caller
   path; projected record copy-back reconstructs each enclosing nominal record,
   while projected indexed copy-back stores an exact path-keyed sequence overlay
   and emits recursive leaf equations for every enclosing record sibling. If a
   later record call consumes a path with descendant overlays, those overlays
   are structurally rebased onto the record formal and then back onto the caller
   path after the callee transition.

The return cell and field copy-back are one transition. A later call can consume
the bound return value, and body TV relates that data flow to the callee's
independently interpreted source tail rather than its authored contract. Nested
writes remain exact because changing a nested leaf reconstructs its
containing direct field before that field is copied back. Copying the complete
field inventory and complete sequence equality make collateral mutations
observable. Sequence writes use exact chained `Seq::update` states, so later
reads and subsequent calls observe the program-point sequence rather than the
entry or final view. A repeated actual root, shared/exclusive overlap, nominal or
indexed pointee/capacity mismatch, missing field/state, unsupported body form,
or recursive effect cycle returns `Unsupported`; none can silently become a
no-op effect. Access-path aliasing uses a structural prefix rule, so
`outer.left` and `outer.right` may coexist while `outer`, `outer.left`, and
`outer.left.child` overlap pairwise. A mutable record, slice, or array peer may be reborrowed as the
shared actual when it is a different root; the snapshot observes any preceding
caller mutation rather than the peer's entry state.

The projected indexed state is deliberately a sequence overlay rather than a
fabricated conversion from `Seq<T>` back into `[T; N]`. Final-state equations
recurse through the independently parsed record declarations: the changed array
path compares to the overlay, and every scalar or untouched array sibling
compares to its exact current native value. A later index read, projected call,
or terminal indexed assignment consults the overlay. Replacing the array or an
enclosing record invalidates overlays at or below that path. When both arms of
a conditional invalidate the same overlay, their exact native values merge;
one-arm-only creation or invalidation fails closed instead of retaining a stale
pre-branch sequence. A later direct or projected record call snapshots every
native scalar/record field while independently rebasing descendant overlays
onto its mutable or shared record formals. Indexed reads and writes inside the
callee therefore observe the exact program-point sequence, and mutable
copy-back replaces only the actual record subtree's overlays. If the callee
replaces an array or enclosing record, the corresponding overlay is removed and
the exact native replacement becomes authoritative. This is leafwise state
composition, not a fabricated `Seq<T>`-to-`[T; N]` conversion. A final
finite-record result is admitted when it is an exact constructor or access path:
every scalar leaf is compared directly and every overlaid array leaf is compared
by its complete sequence view. An explicitly typed intermediate finite-record
`let` may use the same constructor/access-path subset. The independent state
snapshots its direct native fields and rebases each descendant sequence overlay
under the new local root. A later typed local-to-local binding, direct scalar
write, terminal fixed-array write, whole-local constructor/access-path
reassignment, field/index read, or final leafwise result therefore observes that
snapshot rather than the source root's later state. Immutable logical record
roots reject writes.

The independent theory still never fabricates a native array or enclosing
record from a logical sequence. Extracting an overlaid field as `[T; N]`,
passing the logical record through an arbitrary by-value expression or call,
matching it as a native aggregate, or otherwise escaping the controlled typed
binding and leafwise-result paths remains fail-closed.

## Production and expression fidelity

The production obligation includes the ordinary lowered callee and executes the
real generated call. Mutable callees do not receive a pure
`when_used_as_spec` surrogate. For compositional caller TV, Forge adds the
independently reconstructed exact result/state predicate to that exact lowered
callee in the obligation unit; Verus must prove it from the emitted body before
the caller may use it. This closes complete-sequence composition even when an
authored pointwise contract is logically sufficient but not automatically
extensional, without assuming a parallel implementation. Consequently,
deleting a call, changing an argument, reordering dependent calls, or changing
an unmentioned collateral element changes or fails a Verus obligation.

Per-expression TV preserves a named-record borrow as `&mut Name`. An effectful
mutable-call initializer is intentionally owned by whole-body TV: placing an
exec-mode call in a pure `ensures` expression is invalid Verus and would separate
its return from its state effect. The strict inventory therefore omits only that
initializer's pure exec row while requiring the faithful body row; surrounding
pure initializers and the tail retain their normal exec-TV rows. A field read in
the independent postcondition selects `final(root).field`, while the production
wrapper body executes the ordinary `root.field` read. This is the exact Verus
phase distinction for a wrapper that does not itself mutate the already-reached
program-point state; using the unselected mutable name is an adversarial failure.

## Assurance and acceptance

The focused real-Verus suites prove two dependent mutable-record calls, direct
let-bound result flow with exact post-state, a distinct two-root call, fixed-
array and mutable-slice result calls with complete sequence post-state, and mixed
shared/mutable record and indexed snapshot-result composition. They additionally
prove an arbitrary-depth projected mutable call followed by a shared/mutable
sibling call over the caller's current reconstructed state, and arbitrary-depth
projected fixed-array mutable/shared calls whose current sequence flows between
disjoint record siblings while every enclosing scalar leaf is framed. They
reject a wrong/discarded result, nested result use, dropped second call, wrong
argument, missing collateral callee frame, duplicate exclusive alias, recursive
effect cycle, shared/exclusive overlap, exact array capacity/type mismatch, and
missing `final` field selector. Projected tests reject wrong nominal types,
wrong borrow modes, equal-path overlap, and ancestor/descendant overlap before
Verus, while admitting disjoint siblings, a projected root beneath an
independently shared outer record, and repeated shared/shared projections.
Forge's corpus body-TV test derives the effect from source rather than accepting
a hand-authored model, proves both a shared caller input and the current snapshot
of a separately mutable peer, and rejects overlap before invoking Verus.

The policy-free `mutable_call_effect.th` fixture copies from a distinct shared
opaque record through generated mixed-borrow logic, passes that result into the
next generated mutable call, returns it to a downstream consumer, proves the
shared source remains unchanged at runtime, builds for the generic freestanding
target at strict L3, and replays the receipt. Every reachable member and
every contract, exec, body, and wrapper TV row must be L3/faithful. Bound source
tampering invalidates verification. There is no bodyless declaration in this
increment, so every added application primitive is L3; no L1 exception is used.

The sibling `mutable_indexed_call_effect.th` fixture executes a generated
fixed-array mutation/result call from a linked consumer, checks the untouched
element at runtime, requires every reachable receipt and TV row at L3/faithful,
replays the receipt, and rejects source tampering. Forge corpus tests derive
both fixed-array and slice frames from ordinary Thermite signatures rather than
accepting hand-authored metadata.

The `mixed_indexed_call_effect.th` fixture writes one mutable array, then
reborrows that current state as a nonoverlapping shared input to update another
array. Its linked consumer checks both complete generated transitions; every
reachable row is L3/faithful, replay succeeds, and bound-source tampering is
rejected. Shared/shared sequence aliases remain harmless, while an
exclusive/shared overlap rejects before Verus.

The `projected_record_call_effect.th` fixture performs a generated write through
`&mut outer.pair.left`, then snapshots that current state through
`&outer.pair.left` while mutating the disjoint sibling
`&mut outer.pair.right`. Its strict freestanding receipt contains only L3
members, replay succeeds, the generated code is linked and executed by a
downstream consumer, all enclosing guards and tags are preserved, and changing
the bound projected borrow invalidates verification.

The `projected_indexed_call_effect.th` fixture performs a generated mutation
through `&mut outer.left.slots`, snapshots that current sequence through
`&outer.left.slots`, and mutates `&mut outer.right.slots`. Its strict
freestanding receipt has 51 faithful translation-validation rows and only L3
reachable members. Replay, linked downstream execution, untouched array slots,
guard/tag preservation, and projected-borrow tamper rejection are mandatory.

The `record_after_indexed_call_effect.th` fixture mutates a projected fixed
array, passes the enclosing record through a generated mutable record call,
then snapshots that current record through a generated shared record call while
mutating a disjoint sibling. It also returns a finite record literal whose array
field is the current projected sequence overlay. A second exported transition
binds that current record into immutable and mutable typed intermediate locals,
updates the local fixed-array and scalar fields, and returns the independent
snapshot without mutating unrelated caller state. Its strict freestanding
receipt has 88 faithful translation-validation rows and only L3 reachable
members. Replay, total-wrapper guard fidelity, linked downstream execution,
exact returned array/scalar leaves, both sequence transitions, local snapshot
independence, scalar sibling framing, and record-actual tamper rejection are
mandatory.

This is reusable language and proof machinery. It adds no scheduler, allocator,
boot path, firmware runtime, architecture implementation, or kernel artifact.

## Residual boundary

The frozen subset still excludes implicit field borrowing, an array-element
actual such as `&mut slots[i]`, arbitrary native whole-record/array
materialization and by-value call/match use after a descendant sequence overlay,
nested result use inside
arithmetic/conditions/arguments/assignments/tails, untyped
result bindings, recursive mutable effects, mutable enum payloads, calls inside
the record-loop theory, dynamically quantified aggregate frames, and concurrent
or atomic machine effects. Those require separate alias, evaluation-order,
storage, loop, and machine-refinement primitives.
