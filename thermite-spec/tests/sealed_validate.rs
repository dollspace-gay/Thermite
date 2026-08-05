//! Basis Stage 6 — the `#[sealed]` ABSTRACTION-BARRIER validator rule
//! (`.design/basis/06-provenance-and-sinks.md` REQ-8; blocker #77). A `#[sealed]`
//! clean/capability type is boundary-only-mintable by default. The explicit
//! `#[sealed("factory")]` form authorizes exactly one named, bodyful, checked
//! Thermite function to construct it while every other literal remains rejected.
//! A plain struct's literal is accepted as before. Expectations are hand-derived
//! from REQ-8/AC-7 (R-CHAR-3), never read back from validator output.

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
    // from a `Tainted` payload, outside the `parameterize` door.
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
    // The door (`parameterize`) is a `#[boundary]` with a foreign body — no
    // in-language `StructLit` — so the seal does not block it. The safe path mints
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
    // A non-`#[sealed]` struct's `StructLit` is accepted as before — the
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
    // With no `#[sealed]` struct declared, the barrier set is empty — every
    // `StructLit` is accepted (the non-IFC corpus is unchanged, AC-6).
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

#[test]
fn an_explicit_checked_factory_may_construct_its_sealed_type() {
    let src = r#"
#[sealed("mint_cap")] struct Cap { raw: u64 }

fn mint_cap(raw: u64) -> Cap
  req true
  ens result.raw == raw
  fx pure
{
  Cap { raw: raw }
}

fn use_factory(raw: u64) -> u64
  req true
  ens result == raw
  fx pure
{
  mint_cap(raw).raw
}
"#;
    assert!(
        validate_src(src).is_ok(),
        "the exact named bodyful Thermite factory must be allowed to construct the seal"
    );
}

#[test]
fn a_second_function_cannot_launder_through_the_factory_exception() {
    let src = r#"
#[sealed("mint_cap")] struct Cap { raw: u64 }

fn mint_cap(raw: u64) -> Cap
  req true
  ens result.raw == raw
  fx pure
{
  Cap { raw: raw }
}

fn counterfeit(raw: u64) -> Cap
  req true
  ens result.raw == raw
  fx pure
{
  Cap { raw: raw }
}
"#;
    let errs = validate_src(src).expect_err("a non-factory sealed literal must be rejected");
    assert!(
        errs.iter().any(|e| matches!(
            e,
            SpecError::SealedConstruction { name, .. } if name == "Cap"
        )),
        "expected the foreign construction to be rejected, got {errs:?}"
    );
}

#[test]
fn a_missing_sealed_factory_is_rejected() {
    let src = "#[sealed(\"mint_cap\")] struct Cap { raw: u64 }\n";
    let errs = validate_src(src).expect_err("an absent factory must reject the program");
    assert!(errs.iter().any(|e| matches!(
        e,
        SpecError::InvalidSealedFactory { name, factory, .. }
            if name == "Cap" && factory == "mint_cap"
    )));
}

#[test]
fn a_boundary_cannot_masquerade_as_a_checked_sealed_factory() {
    let src = r#"
#[sealed("mint_cap")] struct Cap { raw: u64 }

#[boundary("machine::mint_cap")]
fn mint_cap(raw: u64) -> Cap
  req true
  ens result.raw == raw
  fx platform(memory)
  ;
"#;
    let errs = validate_src(src).expect_err("a bodyless boundary is not a checked factory");
    assert!(errs.iter().any(|e| matches!(
        e,
        SpecError::InvalidSealedFactory { name, factory, .. }
            if name == "Cap" && factory == "mint_cap"
    )));
}

#[test]
fn a_factory_returning_another_type_is_rejected() {
    let src = r#"
#[sealed("mint_cap")] struct Cap { raw: u64 }

fn mint_cap(raw: u64) -> u64
  req true
  ens result == raw
  fx pure
{
  raw
}
"#;
    let errs = validate_src(src).expect_err("factory return type must match the seal exactly");
    assert!(errs.iter().any(|e| matches!(
        e,
        SpecError::InvalidSealedFactory { name, factory, .. }
            if name == "Cap" && factory == "mint_cap"
    )));
}
