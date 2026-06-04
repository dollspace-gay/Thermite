//! Divergence tests pinned by the acto-critic adversarial audit of commit
//! `a06bf16` (thermite-spec: SpecTherm registry + validator, #2).
//!
//! These are FAILING tests that pin places where the validator diverges from
//! its governing contract `.design/spec/spectherm-combinators.md` + thermite
//! -design.md §4.2. Expected behavior traces to the design doc / corpus
//! (R-CHAR-3); none of these expected values are read back from the validator's
//! own output.
//!
//! Each test is `#[ignore]`d with its tracking blocker; removing the `#[ignore]`
//! and going green is the fixer's job (R-DEFER-3).

use thermite_spec::{validate, SpecError};

/// Parse a `.th` source and assert it parsed clean — a parse failure would mean
/// thermite-syntax broke, not the divergence under test.
fn parse_clean(src: &str) -> thermite_syntax::Program {
    let r = thermite_syntax::parse(src);
    assert!(
        r.errors.is_empty(),
        "probe source failed to PARSE (thermite-syntax): {:?}",
        r.errors
    );
    r.program
}

// ---------------------------------------------------------------------------
// DIVERGENCE 1 (cardinal): the conformance corpus `binary_search.th` does NOT
// validate clean.
//
// Authority: .design/spec/spectherm-combinators.md AC-2 — "Validating the
// parsed `conformance/sum.th` and `conformance/binary_search.th` returns
// `Ok(())`". Also goal.md: the corpus is a hand-certified external truth the
// toolchain MUST satisfy.
//
// Actual: validate(binary_search) returns
//   Err [UnknownCombinator { name: "Some", .. }]
// because the validator walks the `fn` BODY's statement expressions through the
// full SpecTherm cage rule (walk_block -> walk_stmt -> walk_expr), and the body
// statement `return Some(mid);` is an `Expr::Call { callee: Path(["Some"]) }`
// that the cage rejects as an unknown combinator.
//
// Root cause / design contradiction: REQ-3 enumerates the contract positions as
// `Contract.req`/`ens`, `LoopNode.invs`/`dec`, and `SpecFnItem.body` — a `fn`
// BODY is NOT a contract position (only the loops *within* it are, for their
// inv/dec). The validator's own `walk_block` doc-comment states "a `fn` body is
// not itself a contract position — we only descend to surface nested loop
// contracts", but the code applies `walk_expr` (the cage) to every fn-body
// statement expression regardless.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "divergence: corpus binary_search.th fails AC-2 (fn-body Some() rejected as combinator); tracking blocker filed"]
fn divergence_corpus_binary_search_validates_clean() {
    // AC-2 expected value: Ok(()) for the hand-certified corpus program.
    let src = include_str!("../../conformance/binary_search.th");
    let program = parse_clean(src);
    let result = validate(&program);
    assert!(
        result.is_ok(),
        "AC-2: conformance/binary_search.th MUST validate clean (Ok(())), got {:?}",
        result.err()
    );
}

#[test]
#[ignore = "divergence: enum constructor call in fn body wrongly cage-checked; tracking blocker filed"]
fn divergence_enum_ctor_in_fn_body_is_not_a_contract_position() {
    // §4.1: `Some`/`None` are sanctioned built-in constructors. A `fn` BODY is
    // not a contract position (REQ-3 enumerates the positions; a fn body is not
    // among them — only its nested loop inv/dec are). `return Some(0);` in a
    // body must NOT be subjected to the combinator cage.
    let src = "fn f(xs: &[u32]) -> u32 req true ens result == 0 fx pure { return Some(0); 0 }";
    let program = parse_clean(src);
    let result = validate(&program);
    assert!(
        result.is_ok(),
        "an enum-constructor call in a (non-contract) fn body must not be cage-checked; got {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// DIVERGENCE 2 (over-fitting): an arbitrary, non-built-in method call in a
// contract position is silently ACCEPTED.
//
// Authority: .design/spec/spectherm-combinators.md REQ-3(c) — a contract admits
// only "the bounded built-in `MethodCall`s the grammar admits (e.g. `xs.len()`)"
// — and REQ-4(iv): "a construct the contract sublanguage forbids that
// nonetheless parsed" must be rejected with `ForbiddenCall`. thermite-design.md
// §4.2 "locks the cage": a contract may use ONLY the frozen vocabulary.
//
// Actual: `req xs.frobnicate()` validates clean (Ok(())). The validator's
// `Expr::MethodCall` arm accepts ANY method name structurally, only recursing
// into operands — it never checks the method name against the bounded built-in
// set. The oracle only ever exercises `.len()`, so this leak is invisible to
// the existing fixtures (the exact over-fitting risk).
// ---------------------------------------------------------------------------

#[test]
#[ignore = "divergence: arbitrary method-call name in contract silently accepted (cage leak); tracking blocker filed"]
fn divergence_unknown_method_call_in_contract_is_rejected() {
    // REQ-3(c)/REQ-4(iv): `xs.frobnicate()` is not a grammar built-in method;
    // the cage must reject it (expected: a ForbiddenCall, the REQ-4(iv) variant).
    let src = "fn f(xs: &[u32]) -> u32 req xs.frobnicate() ens result == 0 fx pure { 0 }";
    let program = parse_clean(src);
    let result = validate(&program);
    let errors = match result {
        Ok(()) => panic!(
            "REQ-3(c)/REQ-4(iv): an arbitrary method call `xs.frobnicate()` in a contract \
             must be REJECTED, but it validated clean"
        ),
        Err(e) => e,
    };
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SpecError::ForbiddenCall { .. })),
        "expected a ForbiddenCall for the non-built-in method `frobnicate`, got {errors:?}"
    );
}
