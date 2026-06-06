//! Critic divergence pins — Basis Stage 1a ADT SURFACE (`.design/basis/01-adts.md`).
//!
//! Each test below pins a divergence between the parser as committed at
//! `32fab6b` and the authority (`.design/basis/01-adts.md` REQs + the parse
//! oracle facts). Expected shapes are hand-derived from the design (R-CHAR-3),
//! never read back from the parser's output.

use thermite_syntax::{parse, Expr, Item};

/// DIVERGENCE — the no-struct-literal disambiguation leaks from a contract
/// clause into a `match` arm BODY.
///
/// Authority: `.design/basis/01-adts.md` REQ-2 — "a struct-variant construction
/// is a NEW `Expr::StructLit`" — and REQ-4 — "`match` in expression position".
/// A `match` ARM BODY is in VALUE position, so a struct-literal construction
/// (`Point { x: 1 }`) there MUST parse, exactly as it does in any other value
/// position (e.g. a `let` initializer, which `tests/adt_parse.rs`
/// `bank_account_parses_struct_with_inv_and_struct_lit` confirms parses).
///
/// The parser threads a `no_struct_literal` context (`parser::Parser`) through
/// the `match`/`if`/`while` HEAD so `match s { … }` reads `{` as the arm block
/// rather than `s { … }` as a struct literal. A contract clause is also a
/// no-struct-literal head (`parse_clause` -> `with_no_struct_literal`). But
/// `parse_match` does NOT re-enable struct literals for its arm bodies
/// (only `parse_call_args`/`parse_index_arg`/the paren primary do, via
/// `with_struct_literal`). So when a `match` is the BARE expression of a
/// contract clause — exactly the shape of `binary_search.th`'s
/// `ens match result { … }` — the suppression set by `parse_clause` LEAKS into
/// the arm bodies, and a struct-literal arm body fails to parse.
///
/// This is the no-struct-literal-context mis-parse the Stage 1a audit flags as
/// highest-value: a value-position struct construction that the design admits
/// (REQ-2) is rejected purely because of an enclosing head context.
///
/// Tracking: #64
#[test]
fn divergence_clause_match_arm_body_struct_lit_parses() {
    // A `fn` whose `ens` clause is a bare `match` (a no-struct-literal head) and
    // whose arm bodies construct a struct in VALUE position.
    let src = "\
struct Point { x: u64, }
enum E { A, B, }
fn f(e: E) -> bool
  req true
  ens match e { A => Point { x: 1 }, B => Point { x: 2 }, } is A
  fx  pure
{ true }
";
    let r = parse(src);
    // REQ-2 + REQ-4: the arm-body struct literals are in value position and MUST
    // parse. Authority says clean; the committed parser emits a SyntaxError
    // ("expected `}`, found `{`") because the clause's no-struct-literal context
    // leaks into the arm bodies.
    assert!(
        r.is_clean(),
        "a struct literal in a `match` arm body (value position) inside a \
         contract clause must parse (`.design/basis/01-adts.md` REQ-2/REQ-4); \
         got {:?}",
        r.errors
    );

    // And it must parse to the design-pinned AST: the `ens` clause is an
    // `Expr::Is` whose scrutinee is an `Expr::Match` with two arms, each arm
    // body an `Expr::StructLit` for `Point`.
    let f = match &r.program.items[2] {
        Item::Fn(f) => f,
        other => panic!("item[2] must be the `fn f`, got {other:?}"),
    };
    let ens = &f.contract.ens[0].expr;
    let scrutinee = match ens {
        Expr::Is { scrutinee, .. } => scrutinee.as_ref(),
        other => panic!("ens must be an `Expr::Is`, got {other:?}"),
    };
    let arms = match scrutinee {
        Expr::Match { arms, .. } => arms,
        other => panic!("the `is` scrutinee must be an `Expr::Match`, got {other:?}"),
    };
    assert_eq!(arms.len(), 2, "two arms (A, B)");
    for (i, arm) in arms.iter().enumerate() {
        match &arm.body {
            Expr::StructLit { path, .. } => {
                assert_eq!(
                    path,
                    &vec!["Point".to_string()],
                    "arm[{i}] body constructs a `Point` struct literal (REQ-2)"
                );
            }
            other => panic!("arm[{i}] body must be an `Expr::StructLit`, got {other:?}"),
        }
    }
}
