# Nested aggregate lifecycle primitives

<!--
tier: 3-component
status: shipped
decision: exact nested mutation admits a typed finite-record root, one or more exact field projections, and optionally one final fixed-array index; independent TV reconstructs every enclosing aggregate and rejects every wider aliasing shape
governs:
  - thermite-spec/src/validator.rs
  - thermite-spec/tests/named_record_mutation_validate.rs
  - thermite-lower/tests/nested_aggregate_lifecycle.rs
  - thermite-tv/src/exec_stmt_encode.rs
  - thermite-tv/tests/owned_aggregate_lifecycle_tv.rs
  - forge/src/body_tv.rs
  - forge/tests/body_tv.rs
  - forge/tests/verified_build.rs
  - conformance/verified-build/nested_aggregate_lifecycle.th
audited-content-sha256: a39e2fbb9efe69934e5ee8ae89b7915448d49e3ddba807b3da36d46ced5c2a38 (re-pinned 2026-08-04 after the adjacent ADT lifecycle extension; no kernel policy or implementation was added)
extends:
  - .design/build/owned-aggregate-lifecycle.md
  - .design/build/named-record-lifecycle.md
  - .design/build/kernel-primitives.md
-->

## Decision

Thermite admits nested aggregate assignment only when the source validator and
the independent body denotation can resolve the same complete finite type path.
The root is one explicitly typed writable binding whose dereferenced type is a
finite non-sealed record. The target is:

```text
root.field(.field)*
root.field(.field)*[single_index]
```

This covers direct updates such as `state.inner.value = next` and
`state.slots[index] = next`. The index, when present, is terminal. An
index-then-field target such as `state.records[index].value`, a tuple projection,
an explicit dereference, an inferred record root, a recursive/reference/heap
field, an enum payload, or any computed/aliased receiver remains rejected.

## Independent denotation

Validation resolves each field against its receiver's exact declared type and
requires the terminal index receiver to be a fixed array. Merely finding a field
name somewhere in the program is insufficient.

Body TV starts from the pre-write root value and recursively rebuilds every
enclosing record. At the selected field it either installs the independently
encoded right-hand side or, for a terminal index, installs an exact finite-array
update. Every sibling field projects from the immediately preceding aggregate
value. A later read is evaluated from the rebuilt value, so source order remains
observable.

For an exclusive record parameter, the same reconstruction replaces exactly one
direct root field in the old-to-final lifecycle state. The final obligation still
frames every direct root field, and the reconstructed nested value contains every
untouched nested sibling. For an owned record local, the whole root is rebuilt and
the returned aggregate is compared exactly.

No production-lowering helper is reused by this reference construction. The
production column continues to emit the source lvalue directly; equivalence with
the independently reconstructed state is discharged by real Verus.

The reference preserves bounded target typing explicitly. Arithmetic is encoded
as mathematical integer syntax by the independent expression denotation, then
cast back to the exact declared scalar field or array-element type at assignment.
This prevents a nested finite-array update from becoming unverifiable merely
because its right-hand side is arithmetic while retaining the production
overflow obligation.

## Assurance and acceptance

Completion requires:

1. positive validation for both admitted target forms and negative validation
   for wrong fields, wrong intermediate types, nonterminal indices, sealed or
   non-finite roots, immutable roots, and computed receivers;
2. real-Verus faithful obligations for owned and exclusive-record updates;
3. mutants for dropped writes, wrong field/index/value, reordered dependent
   reads, and collateral sibling changes;
4. a strict freestanding scalar-nested receipt whose generated Thermite logic is
   executed by a downstream consumer;
5. hosted exact array-view evidence for terminal array indices until the
   freestanding no-vstd array-view gap is closed; and
6. every bodyful Thermite item at L3 or L4, with no new boundary.

The shipped battery satisfies these conditions. Validator tests admit both
forms and reject index-then-field, non-array indexing, unknown nested fields,
computed receivers, dereferences, ranges, immutable roots, inferred roots,
sealed roots, and non-finite layouts. The independent TV suite discharges both
owned and exclusive-record forms through real Verus and kills dropped inner or
array writes, wrong indices and values, reordered dependent reads, and nested or
top-level collateral changes. Forge's file-level body walk proves both the
hosted fixed-array form and every body in the freestanding scalar-nested
fixture.

The strict `--target kernel` build contains no `vstd` array view, so the receipt
fixture uses the scalar nested path while the terminal fixed-array path remains
hosted real-Verus evidence. The strict fixture builds only at L3, replays its
receipt, executes the generated nested update from a codegen-pinned downstream
consumer, and rejects bound-source layout tampering. No bodyless declaration or
new assurance exception is introduced.

## Residual boundary

This increment does not admit an index followed by a field, alias construction,
explicit dereference lvalues, tuple projections, enum payload mutation,
recursive/reference/heap fields, or mutation rooted in a computed receiver.
Those shapes require the later alias and enum-payload-mutation lifecycle work;
user-ADT match/results are supplied separately by
`.design/build/adt-match-lifecycle.md`. Record-state loops and mutable-reference
callee effect composition also remain separate increments.

The increment supplies language and verification primitives only. It adds no
allocator, scheduler, firmware, boot path, platform runtime, or kernel policy.
