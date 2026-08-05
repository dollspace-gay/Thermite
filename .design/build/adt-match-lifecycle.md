# ADT result and match lifecycle primitives

<!--
tier: 3-component
status: shipped
decision: executable and contract user-ADT matches are independently denoted from parsed variant ownership and payload shapes; strict builds hand control-flow values to complete body TV rather than duplicating them through leaf exec TV
governs:
  - thermite-tv/src/ref_encode.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/src/obligation.rs
  - thermite-tv/src/lib.rs
  - thermite-tv/tests/adt_match_lifecycle_tv.rs
  - forge/src/body_tv.rs
  - forge/src/contract_tv.rs
  - forge/src/exec_tv.rs
  - forge/src/verified_build.rs
  - forge/tests/verified_composition.rs
  - conformance/verified-composition/probe.th
audited-content-sha256: 6b734ac589317ebbf01a72c3feb1e4570acecddb246f3832073f2642a80af8d1 (re-pinned 2026-08-05 after exact dependency/pre-state TV grounding; ADT semantics remain regression-covered and no kernel was added)
extends:
  - .design/build/owned-aggregate-lifecycle.md
  - .design/build/nested-aggregate-lifecycle.md
  - .design/build/kernel-primitives.md
-->

## Decision

Thermite's independent translation validators admit user-defined enum values and
executable `match` expressions when the complete nominal frame is derivable from
the parsed source program. The frame records each variant's unique enum owner and
its exact unit, tuple, or named-field payload types. It is not inferred from
generated Verus text and does not call production pattern lowering.

The admitted executable pattern set is:

- wildcard and binding patterns;
- scalar literal patterns;
- unit and tuple user-enum variants;
- named-field struct variants, including shorthand and `..`;
- recursively nested versions of those patterns; and
- or-patterns whose alternatives bind the same names.

Match guards and arm bodies are recursively encoded. Slice patterns remain the
special head-fold specification form and are rejected in executable or contract
matches rather than guessed.

## Independent state and scoping semantics

The body denotation substitutes the current closed-form state into the match
scrutinee. Each arm then removes every pattern-bound name from the outer state
environment before substituting its guard and result. Consequently, a payload
binding such as `Tick(value)` denotes the event payload even when an outer local
called `value` already exists. Duplicate bindings, empty or-patterns, and
or-pattern alternatives with different binding sets fail closed inside TV even
if source validation would normally reject them first.

Unqualified user variants are qualified from the parsed owner map. Built-in
Option/Result variants and already-qualified paths retain their source spelling.
Constructor typing uses a separate all-record inventory, distinct from the
narrow finite-record inventory that admits mutation. This lets a match return a
record containing a modeled `Vec` without accidentally making heap-backed record
mutation part of the frozen lifecycle subset.

When an aggregate constructor is moved into an `ensures` reference expression,
the denotation restores every parsed bounded payload type. For example,
`generation + 1` in a `u64` field becomes the bounded `u64` operation rather than
an unintended mathematical `int`. Tuple results are framed recursively enough to
bind native tuple values in contract TV, including tuples of named records and
enums.

## Contract, exec, and body ownership

Contract TV now carries the reachable ADT definitions, tuple signature types,
and user-variant owner map. It independently encodes user tuple/struct patterns,
guards, nested matches, literals, wildcards, bindings, and or-patterns. A changed
arm, predicate, variant, or payload therefore changes the equivalence obligation.

Leaf exec TV remains the validator for pure value expressions. A `match` or `if`
value taken from an in-language body is instead owned by the stronger complete
body-TV state obligation. The strict expected-row inventory applies the same
handoff rule, so it cannot demand a second leaf row or silently accept a missing
body row. Export guards have no body owner and do not receive this exemption.

Body TV compares the production body's complete result with the independently
substituted match value. It observes branch selection, variant selection, every
payload, aggregate fields, source ordering, and pattern shadowing. Reachable
callee reference definitions receive the same enum/constructor frame.

## Assurance and acceptance

Completion of this increment requires:

1. real-Verus faithful body obligations for a user tuple variant, a unit variant,
   a struct variant, and an aggregate tuple result;
2. real-Verus rejection of wrong-variant, wrong-payload, dropped-transition,
   collateral-field, wrong-arm, and pattern-capture mutants;
3. faithful contract TV for nested user-enum and struct-variant matches;
4. a strict L3 freestanding composition receipt containing faithful contract and
   body rows with the control-value exec handoff represented exactly once;
5. replay, deterministic artifact construction, downstream execution of the
   generated Thermite transition, and source/shell tamper rejection; and
6. no bodyful application primitive below L3 and no new frozen boundary.

The focused `adt_match_lifecycle_tv` battery supplies the faithful and adversarial
real-Verus obligations. The policy-free `probe.th` composition fixture supplies
the strict receipt, replay, deterministic build, linked runtime, privacy, and
tamper evidence. Its body is the generated Thermite match; there is no Rust
parallel implementation producing an expected marker.

## Residual boundary

This increment does not add enum-payload lvalue mutation or slice-pattern
execution. Record-state loop fixpoints are supplied separately by
`.design/build/record-state-loops.md`; exact direct finite-record mutable-call
effects and their pairwise-distinct alias rule are supplied by
`.design/build/mutable-call-effects.md`. Wider alias and mutable-call forms remain
separate aggregate-lifecycle increments. It also
does not add an allocator, scheduler, IPC policy, boot image, firmware path,
architecture implementation, or any other kernel.

Every bodyful semantic added here is discharged at L3. No L1 declaration is
introduced; the platform-only exception class is unchanged.
