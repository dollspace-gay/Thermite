//! FINAL re-audit (loop 3 / issue #2) of the #35/#36 fix at HEAD `eae38c3`.
//!
//! The #35 fix replaced fn-body caging with a STRUCTURAL traversal
//! (`scan_block_for_loops` / `scan_stmt_for_loops` / `scan_expr_for_loops`)
//! that descends a `fn` body ONLY to find nested `LoopNode`s and cage each
//! loop's `invs`/`dec`, leaving the body's surface expressions un-caged. The
//! #36 fix bounded the contract-position `MethodCall` allowlist to `{len}`.
//!
//! This file is the adversarial probe for the obvious failure mode of that
//! refactor: a structural traversal that fails to descend into some fn-body
//! position the corpus never exercises would let a bogus combinator in a
//! nested loop's `inv`/`dec` SLIP THROUGH — a cage HOLE the old over-caging
//! never had. Each test below nests a loop whose `inv` calls the UNKNOWN
//! combinator `frobnicate` (NOT in the frozen §4.2 set / registry) in a
//! distinct fn-body position and asserts the cage STILL rejects it. The
//! complement tests assert ordinary surface code in those same positions is
//! NOT rejected (the #35 intent).
//!
//! Authority (R-CHAR-3 — expected values are NOT read back from the validator):
//!   - .design/spec/spectherm-combinators.md REQ-3 (cage = req/ens, LoopNode
//!     invs/dec, SpecFnItem.body) + REQ-4 (i) (UnknownCombinator) + REQ-3(c)/
//!     REQ-4(iv) (bounded built-in MethodCall -> ForbiddenCall).
//!   - thermite-design.md §4.2 — "a fixed library of bounded combinators";
//!     "locks the cage". `frobnicate` is not in that frozen set, so a contract
//!     position holding it MUST reject; `sorted`/`forall_*` ARE in it.
//!   - conformance/binary_search.th — the corpus combinator names
//!     (`sorted`, `forall_below`, `forall_from`) used in the well-formed cases.
//!
//! These are regression pins: they lock the cage's behavior across the four
//! body positions the corpus does not exercise (if-then, if-else, loop-in-loop,
//! if-EXPR). A future edit that reintroduces a traversal hole turns one of these
//! red.

use thermite_spec::{validate, SpecError};

/// Parse a `.th` source and assert it parsed clean — a parse failure would mean
/// thermite-syntax broke, not the cage behavior under test.
fn parse_clean(src: &str) -> thermite_syntax::Program {
    let r = thermite_syntax::parse(src);
    assert!(
        r.errors.is_empty(),
        "probe source failed to PARSE (thermite-syntax), not a cage result: {:?}",
        r.errors
    );
    r.program
}

/// True iff the errors contain an `UnknownCombinator` naming `frobnicate` — the
/// §4.2-cage rejection of a bogus combinator in a contract position.
fn rejects_frobnicate(errs: &[SpecError]) -> bool {
    errs.iter()
        .any(|e| matches!(e, SpecError::UnknownCombinator { name, .. } if name == "frobnicate"))
}

// ===========================================================================
// The bogus combinator MUST be caught in every nested-loop fn-body position.
// ===========================================================================

/// Loop nested inside an `if`'s THEN branch (`Stmt::If.then`). The traversal
/// must descend `scan_stmt_for_loops` -> `Stmt::If` -> `scan_block_for_loops`.
#[test]
fn bogus_inv_in_if_then_branch_is_rejected() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  if xs.len() > 0 {
    loop
      inv frobnicate(xs)
      dec xs.len()
    { return 0; }
  }
  0
}
"#;
    let errs = validate(&parse_clean(src)).expect_err("bogus combinator in nested loop inv");
    assert!(
        rejects_frobnicate(&errs),
        "cage HOLE: bogus combinator in a loop inside an `if`-then branch slipped through: {errs:?}"
    );
}

/// Loop nested inside an `if`'s ELSE branch (`Stmt::If.else_`).
#[test]
fn bogus_inv_in_if_else_branch_is_rejected() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  if xs.len() > 0 { return 1; } else {
    loop
      inv frobnicate(xs)
      dec xs.len()
    { return 0; }
  }
  0
}
"#;
    let errs = validate(&parse_clean(src)).expect_err("bogus combinator in else-branch loop inv");
    assert!(
        rejects_frobnicate(&errs),
        "cage HOLE: bogus combinator in a loop inside an `if`-else branch slipped through: {errs:?}"
    );
}

/// Inner loop nested inside an OUTER loop's body; the INNER loop's `inv` is
/// bogus while the outer's is well-formed. The traversal must descend a loop
/// body (`scan_block_for_loops(&loop_node.body)`) to reach the inner loop.
#[test]
fn bogus_inner_loop_inv_nested_in_outer_loop_is_rejected() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  loop
    inv sorted(xs)
    dec xs.len()
  {
    loop
      inv frobnicate(xs)
      dec xs.len()
    { return 0; }
    return 1;
  }
  0
}
"#;
    let errs = validate(&parse_clean(src)).expect_err("bogus inner-loop inv");
    assert!(
        rejects_frobnicate(&errs),
        "cage HOLE: bogus combinator in a loop nested inside another loop's body slipped through: {errs:?}"
    );
}

/// Loop nested inside an `if`-EXPRESSION (value position, `Expr::If`) via a
/// `let` initializer. The traversal must descend `scan_expr_for_loops` ->
/// `Expr::If` -> `scan_block_for_loops` (the expression arm, distinct from the
/// statement arm above).
#[test]
fn bogus_inv_in_if_expr_branch_is_rejected() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  let y: usize = if xs.len() > 0 {
    loop
      inv frobnicate(xs)
      dec xs.len()
    { return 0; }
    1
  } else { 2 };
  y
}
"#;
    let errs = validate(&parse_clean(src)).expect_err("bogus inv in if-expr branch loop");
    assert!(
        rejects_frobnicate(&errs),
        "cage HOLE: bogus combinator in a loop inside an `if`-EXPRESSION branch slipped through: {errs:?}"
    );
}

/// A bogus combinator in a loop's `dec` (not just `inv`) in a nested position
/// must also reject — `dec` is a contract clause too (REQ-3).
#[test]
fn bogus_dec_in_nested_loop_is_rejected() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  if xs.len() > 0 {
    loop
      inv sorted(xs)
      dec frobnicate(xs)
    { return 0; }
  }
  0
}
"#;
    let errs = validate(&parse_clean(src)).expect_err("bogus dec in nested loop");
    assert!(
        rejects_frobnicate(&errs),
        "cage HOLE: bogus combinator in a nested loop's `dec` slipped through: {errs:?}"
    );
}

// ===========================================================================
// Complement: well-formed nested loops + ordinary surface code in the SAME
// fn-body positions are NOT rejected (the #35 intent — fn bodies aren't caged).
// ===========================================================================

/// A well-formed nested loop (registered combinator `sorted`) validates clean.
#[test]
fn wellformed_nested_loop_validates_clean() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  if xs.len() > 0 {
    loop
      inv sorted(xs)
      dec xs.len()
    { return 0; }
  }
  0
}
"#;
    assert_eq!(
        validate(&parse_clean(src)),
        Ok(()),
        "well-formed nested loop (registered combinator `sorted`) must validate clean"
    );
}

/// Ordinary surface code in fn-body positions — a user-fn call inside an
/// `if` branch — is NOT cage-checked (REQ-3: a `fn` body is not a contract
/// position). The cage rejecting `helper(...)` here would be the #35 over-cage
/// regression.
#[test]
fn surface_user_fn_call_in_if_branch_body_is_not_caged() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  if xs.len() > 0 {
    let z: usize = helper(xs);
    return z;
  }
  0
}
"#;
    assert_eq!(
        validate(&parse_clean(src)),
        Ok(()),
        "fn-body surface call `helper(xs)` must NOT be cage-checked (REQ-3: fn body is not a contract position)"
    );
}

/// A NON-`len` method call (`xs.first()`) in a fn BODY is allowed (body not
/// caged); the allowlist only governs CONTRACT positions.
#[test]
fn non_len_method_in_fn_body_is_not_caged() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  let z: u32 = xs.first();
  0
}
"#;
    assert_eq!(
        validate(&parse_clean(src)),
        Ok(()),
        "fn-body `xs.first()` must NOT be cage-checked (REQ-3: fn body is not a contract position)"
    );
}

// ===========================================================================
// MethodCall allowlist `{len}` (REQ-3(c)/REQ-4(iv)) in CONTRACT positions.
// ===========================================================================

/// `xs.len()` in a CONTRACT position (`req`) is the one allowlisted built-in.
#[test]
fn len_method_in_contract_validates_clean() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req xs.len() <= 10
  ens result <= xs.len()
  fx  pure
{ 0 }
"#;
    assert_eq!(
        validate(&parse_clean(src)),
        Ok(()),
        "`xs.len()` is the allowlisted built-in method (REQ-3(c)) and must validate clean"
    );
}

/// A non-allowlisted method (`xs.first()`) in a CONTRACT position (`ens`) is a
/// `ForbiddenCall` (REQ-4(iv)).
#[test]
fn non_len_method_in_contract_is_forbidden() {
    let src = r#"
fn f(xs: &[u32]) -> u32
  req sorted(xs)
  ens result == xs.first()
  fx  pure
{ 0 }
"#;
    let errs = validate(&parse_clean(src)).expect_err("non-allowlisted method in contract");
    assert!(
        errs.iter().any(|e| matches!(e, SpecError::ForbiddenCall { .. })),
        "non-`len` method `xs.first()` in a contract position must be ForbiddenCall (REQ-4(iv)): {errs:?}"
    );
}

/// A non-allowlisted method (`xs.iter()`) in a nested loop's `inv` (a contract
/// position reached via the structural traversal) is also a `ForbiddenCall` —
/// the allowlist applies wherever the cage applies, including nested loops.
#[test]
fn non_len_method_in_nested_loop_inv_is_forbidden() {
    let src = r#"
fn f(xs: &[u32]) -> usize
  req sorted(xs)
  ens result <= xs.len()
  fx  pure
{
  if xs.len() > 0 {
    loop
      inv xs.iter()
      dec xs.len()
    { return 0; }
  }
  0
}
"#;
    let errs = validate(&parse_clean(src)).expect_err("non-allowlisted method in nested loop inv");
    assert!(
        errs.iter().any(|e| matches!(e, SpecError::ForbiddenCall { .. })),
        "non-`len` method in a NESTED loop inv must be ForbiddenCall (allowlist applies in the cage): {errs:?}"
    );
}
