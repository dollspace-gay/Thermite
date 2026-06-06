//! Basis Stage 6 — the `#[sealed]` ABSTRACTION-BARRIER validator rule
//! (`.design/basis/06-provenance-and-sinks.md` REQ-8; blocker #77). A `#[sealed]`
//! clean/capability type is door-only-mintable: the validator REJECTS any
//! `Expr::StructLit` of a `#[sealed]` struct with `SpecError::SealedConstruction`,
//! anywhere in Thermite code. The `#[boundary]` door is unaffected (its body is
//! foreign/`external_body`, with no in-language `StructLit`), so the safe doored
//! path validates clean. A PLAIN (non-`#[sealed]`) struct's `StructLit` is
//! accepted exactly as before (no regression). Expectations are hand-derived from
//! REQ-8/AC-7 (R-CHAR-3), never read back from the validator's own output.

use thermite_spec::{validate, SpecError};
use thermite_syntax::parse;

/// Parse `src` (asserting it is clean) and validate it.
fn validate_src(src: &str) -> Result<(), Vec<SpecError>> {
    let r = parse(src);
    assert!(r.is_clean(), "fixture must parse clean, got {:?}", r.errors);
    validate(&r.program)
}

#[test]
fn sealed_structlit_launder_is_rejected() {
    // The #77 taint launder: a `Sql` `#[sealed]` clean type minted via `StructLit`
    // from a `Tainted` payload, OUTSIDE the `parameterize` door.
    let src = r#"
struct Tainted { raw: u64 }
#[sealed] struct Sql { stmt: u64 }

#[boundary("ifc::query")] fn query(q: Sql) -> u64
  req true
  ens result == q.stmt
  fx  net(db)
  ;

fn bypass_query(input: Tainted) -> u64
  req true
  ens result == input.raw
  fx  net(db)
{
  query(Sql { stmt: input.raw })
}
"#;
    let errs = validate_src(src).expect_err("a sealed StructLit must be rejected (REQ-8)");
    assert!(
        errs.iter().any(|e| matches!(
            e,
            SpecError::SealedConstruction { name, .. } if name == "Sql"
        )),
        "expected SealedConstruction {{ name: \"Sql\" }}, got {errs:?}"
    );
}

#[test]
fn the_safe_doored_path_validates_clean() {
    // The door (`parameterize`) is a `#[boundary]` with a foreign body — NO
    // in-language `StructLit` — so the seal does NOT block it. The safe path mints
    // no `Sql` literal; it validates clean (REQ-8: the door is the only mint).
    let src = r#"
struct Tainted { raw: u64 }
#[sealed] struct Sql { stmt: u64 }

#[boundary("ifc::parameterize")] fn parameterize(t: Tainted) -> Sql
  req true
  ens result.stmt == t.raw
  fx  pure
  ;

#[boundary("ifc::query")] fn query(q: Sql) -> u64
  req true
  ens result == q.stmt
  fx  net(db)
  ;

fn safe_query(input: Tainted) -> u64
  req true
  ens result == input.raw
  fx  net(db)
{
  query(parameterize(input))
}
"#;
    assert!(
        validate_src(src).is_ok(),
        "the safe doored path carries no sealed StructLit and must validate clean (REQ-8)"
    );
}

#[test]
fn a_plain_struct_literal_is_unaffected() {
    // A non-`#[sealed]` struct's `StructLit` is accepted exactly as before — the
    // seal is opt-in and inert on plain structs (AC-6, no regression).
    let src = r#"
struct Account { balance: u64 }

fn mk(b: u64) -> u64
  req true
  ens result == b
  fx  pure
{
  Account { balance: b }.balance
}
"#;
    assert!(
        validate_src(src).is_ok(),
        "a plain (non-sealed) struct literal must be accepted unchanged (AC-6)"
    );
}

#[test]
fn no_sealed_struct_means_the_rule_is_inert() {
    // With NO `#[sealed]` struct declared, the barrier set is empty — every
    // `StructLit` is accepted (the non-IFC corpus is UNCHANGED, AC-6).
    let src = r#"
struct Sql { stmt: u64 }

fn build(x: u64) -> u64
  req true
  ens result == x
  fx  pure
{
  Sql { stmt: x }.stmt
}
"#;
    assert!(
        validate_src(src).is_ok(),
        "without a #[sealed] declaration the rule is inert — a plain Sql literal is fine (AC-6)"
    );
}
