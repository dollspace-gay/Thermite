//! Adversarial audit of REQ-6 (the flat-closure-fragment rule, issue #40) as
//! landed in commit 4d46f8a (`SpecError::NestedCombinator` + the
//! `Validator::in_combinator_closure` caged-flat flag).
//!
//! Authority: `.design/spec/spectherm-combinators.md` REQ-6 + AC-6/7/8;
//! `thermite-design.md` §4.2 ("No general quantifiers … a fixed library of
//! bounded combinators … Thermite locks the cage"). R-CHAR-3: every expected
//! value below is the design-doc's REQ-6/AC-6/7/8 outcome (REJECT-with-
//! `NestedCombinator` for an anonymous nested combinator; `Ok` for named
//! composition + non-nested siblings), never copied from validator output.
//!
//! These probe the edges the `accept.json`/`reject.json` oracle does NOT cover
//! directly: deeper / other-clause / 3-arg-outer nestings that must STILL
//! reject (under-enforcement), and non-nested siblings + the named-spec-fn-body
//! composition escape that must STAY accepted (over-enforcement / flag leak).
//!
//! AUDIT RESULT: NO DIVERGENCE. Every assertion below holds against 4d46f8a —
//! these are characterization/regression guards, not failing-divergence pins.
//! (The one cosmetic artifact — a co-emitted misleading `ForbiddenCall` on an
//! already-`NestedCombinator`-rejected program — is documented in
//! `nested_combinator_does_not_change_reject_verdict` below; it is diagnostic
//! noise on a correctly-rejected program, not a REQ-6 divergence: AC-6 pins the
//! reject OUTCOME + the cause being present, both satisfied.)

use thermite_spec::{validate, SpecError};

/// Parse a `.th` string, asserting it parses clean (a parse failure would be a
/// thermite-syntax bug, not a thermite-spec divergence).
fn parse(program: &str) -> thermite_syntax::Program {
    let r = thermite_syntax::parse(program);
    assert!(
        r.errors.is_empty(),
        "probe program failed to PARSE (thermite-syntax): {:?}",
        r.errors
    );
    r.program
}

fn has_nested(errs: &[SpecError]) -> bool {
    errs.iter()
        .any(|e| matches!(e, SpecError::NestedCombinator { .. }))
}

// ===========================================================================
// UNDER-ENFORCEMENT probes: each MUST reject with NestedCombinator (REQ-6).
// If any slipped through (Ok), the caged-flat flag would not cover that
// descent — a divergence. All currently reject correctly.
// ===========================================================================

/// A combinator nested TWO levels deep inside `ens`. REQ-6: every anonymous
/// nested combinator is forbidden, at any depth. Expected: Err carrying
/// `NestedCombinator` (the flag is set ONCE on the outermost closure body and
/// kept set through all nested closures, per the design's "caged-flat" walk).
#[test]
fn two_level_nesting_in_ens_rejects() {
    let p = parse(
        "fn f(xs: &[u32], ys: &[u32], zs: &[u32]) -> u32 \
         req true \
         ens forall_in(xs, |x| forall_in(ys, |y| exists_in(zs, |z| z == x))) \
         fx pure { 0 }",
    );
    let errs = validate(&p).expect_err("REQ-6: a 2-level nested combinator must reject");
    assert!(
        has_nested(&errs),
        "REQ-6: nested combinator at depth 2 must yield NestedCombinator; got {errs:?}"
    );
}

/// A nested combinator inside a loop `inv` clause (not `ens`). REQ-6 applies to
/// EVERY contract position's combinator closure body (`req`/`ens`/`inv`/`dec` +
/// spec-fn bodies), not only `ens`. Expected: Err with `NestedCombinator`.
#[test]
fn nested_combinator_in_loop_inv_rejects() {
    let p = parse(
        "fn f(xs: &[u32], ys: &[u32]) -> u32 \
         req true ens true fx pure { \
            let mut i: u32 = 0; \
            loop inv forall_in(xs, |x| exists_in(ys, |y| y == x)) dec 0 { return 0; } \
         }",
    );
    let errs = validate(&p).expect_err("REQ-6: nested combinator in an `inv` closure must reject");
    assert!(
        has_nested(&errs),
        "REQ-6: nested combinator in an `inv` body must yield NestedCombinator; got {errs:?}"
    );
}

/// The OUTER combinator is a 3-arg combinator (`forall_below`). REQ-6 keys on
/// the inner callee resolving via `combinators::lookup`, independent of the
/// outer combinator's arity. Expected: Err with `NestedCombinator`.
#[test]
fn nested_combinator_under_three_arg_outer_rejects() {
    let p = parse(
        "fn f(xs: &[u32], ys: &[u32]) -> u32 \
         req true \
         ens forall_below(xs, 1, |x| forall_in(ys, |y| y > x)) \
         fx pure { 0 }",
    );
    let errs = validate(&p).expect_err("REQ-6: nested combinator under a 3-arg outer must reject");
    assert!(
        has_nested(&errs),
        "REQ-6: nested combinator under `forall_below` must yield NestedCombinator; got {errs:?}"
    );
}

/// A nested combinator inside a `spec fn` BODY's combinator closure. A spec-fn
/// body is itself a fully-caged contract position (REQ-3); REQ-6's flag must be
/// set when descending its combinator closures too. Expected: Err with
/// `NestedCombinator`.
#[test]
fn nested_combinator_in_spec_fn_body_rejects() {
    let p = parse(
        "spec fn bad(xs: &[u32], ys: &[u32]) -> bool dec 0 \
            { forall_in(xs, |x| exists_in(ys, |y| y == x)) } \
         fn f(xs: &[u32]) -> u32 req true ens true fx pure { 0 }",
    );
    let errs = validate(&p)
        .expect_err("REQ-6: nested combinator inside a spec-fn body closure must reject");
    assert!(
        has_nested(&errs),
        "REQ-6: nested combinator in a spec-fn body must yield NestedCombinator; got {errs:?}"
    );
}

// ===========================================================================
// OVER-ENFORCEMENT / FLAG-LEAK probes: each MUST stay accepted (REQ-6 narrows
// the accept set ONLY inside a combinator closure body, and the flag must be
// restored after that body). If any rejected, the flag leaked / over-enforced
// — a divergence. All currently accept correctly.
// ===========================================================================

/// A top-level combinator clause AFTER a combinator-with-closure clause in the
/// same contract. Neither is nested; both are top-level `ens` combinators
/// (REQ-3(a)). Confirms the flag is RESTORED after the first Pred descent and
/// does not leak into the sibling clause. Expected: Ok.
#[test]
fn top_level_combinator_after_closure_combinator_accepts() {
    let p = parse(
        "fn f(xs: &[u32], ys: &[u32]) -> u32 \
         req true \
         ens forall_in(xs, |x| x > 0) \
         ens sorted(ys) \
         fx pure { 0 }",
    );
    validate(&p).expect("REQ-6: a top-level combinator after a closure-combinator must stay Ok");
}

/// Two sibling top-level `forall_in` clauses, the FIRST carrying a closure.
/// The second's closure must be checked independently (flag restored), so its
/// own flat body validates. Expected: Ok (no leak across clauses).
#[test]
fn two_sibling_closure_combinators_accept() {
    let p = parse(
        "fn f(xs: &[u32], ys: &[u32]) -> u32 \
         req true \
         ens forall_in(xs, |x| x > 0) \
         ens forall_in(ys, |y| y < 0) \
         fx pure { 0 }",
    );
    validate(&p).expect("REQ-6: two sibling closure-combinators must both stay Ok");
}

/// THE NAMED-COMPOSITION ESCAPE (the honest caveat of REQ-6, the most likely
/// over-enforcement bug). A `spec fn` whose BODY itself calls a combinator is a
/// general contract position (REQ-3(a)) — its combinator use is ACCEPTED. The
/// flag must NOT leak into spec-fn-body validation. Expected: Ok.
#[test]
fn spec_fn_body_calling_a_combinator_accepts() {
    let p = parse(
        "spec fn all_pos(xs: &[u32]) -> bool dec 0 { forall_in(xs, |x| x > 0) } \
         fn f(xs: &[u32]) -> u32 req true ens all_pos(xs) fx pure { 0 }",
    );
    validate(&p).expect("REQ-6 named caveat: a spec-fn body may itself call a combinator (Ok)");
}

/// The full named-composition chain: a combinator closure body calls a NAMED
/// `spec fn` whose own body quantifies via a combinator. REQ-6 explicitly
/// sanctions this (named, `dec`-measured composition); only ANONYMOUS nested
/// combinators are forbidden. Expected: Ok. This is the precise distinction
/// between AC-6 (reject) and AC-7 (accept).
#[test]
fn named_spec_fn_quantifier_called_from_closure_accepts() {
    let p = parse(
        "spec fn all_pos(ys: &[u32]) -> bool dec 0 { forall_in(ys, |x| x > 0) } \
         fn f(ys: &[u32]) -> u32 req true ens forall_in(ys, |s| all_pos(ys)) fx pure { 0 }",
    );
    validate(&p)
        .expect("REQ-6: a closure body MAY call a named spec fn that internally quantifies (Ok)");
}

/// A named `spec fn` call inside a closure body (AC-7 canonical). Expected: Ok.
#[test]
fn named_spec_fn_call_in_closure_accepts() {
    let p = parse(
        "spec fn is_even(x: u32) -> bool dec 0 { x > 0 } \
         fn f(xs: &[u32]) -> u32 req true ens forall_in(xs, |x| is_even(x)) fx pure { 0 }",
    );
    validate(&p).expect("AC-7: a named spec-fn call in a closure body must stay Ok");
}

// ===========================================================================
// Verdict-integrity + no-panic guards.
// ===========================================================================

/// The reject VERDICT for a nested combinator is unchanged by the co-emitted
/// secondary diagnostics: the program rejects and `NestedCombinator` is among
/// the causes (AC-6). (Audit note: the validator also co-emits a misleading
/// `ForbiddenCall` "a closure may appear only as a combinator predicate
/// argument" for the nested combinator's own legitimate Pred closure — this is
/// diagnostic noise on an already-rejected program, NOT a REQ-6 divergence, so
/// it is documented rather than pinned as a failing test. AC-6 pins the reject
/// outcome + the cause's presence, both of which hold.)
#[test]
fn nested_combinator_reject_verdict_holds() {
    let p = parse(
        "fn f(xs: &[u32], ys: &[u32]) -> u32 req true \
         ens forall_in(xs, |x| exists_in(ys, |y| y == x)) fx pure { 0 }",
    );
    let errs = validate(&p).expect_err("AC-6: the canonical nested combinator must reject");
    assert!(
        has_nested(&errs),
        "AC-6: the reject must carry NestedCombinator; got {errs:?}"
    );
}

/// REQ-5 preserved under the flag: a deep nest of closure-combinators is a
/// structured Err (never a panic / stack overflow). Expected: Err, no panic.
#[test]
fn deeply_nested_closures_do_not_panic() {
    let mut inner = String::from("x > 0");
    for _ in 0..25 {
        inner = format!("forall_in(xs, |x| {inner})");
    }
    let prog = format!("fn f(xs: &[u32]) -> u32 req true ens {inner} fx pure {{ 0 }}");
    let r = thermite_syntax::parse(&prog);
    if r.errors.is_empty() {
        // Validation must terminate with a structured Err (nested combinators),
        // never overflow the stack.
        let res = validate(&r.program);
        assert!(
            res.is_err(),
            "REQ-6: a deep nest of combinator-closures must be a structured Err"
        );
    }
    // (If thermite-syntax's own recursion guard rejected the parse first, the
    // validator is never reached — still no panic. Either way: no overflow.)
}
