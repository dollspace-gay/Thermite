//! Basis Stage 6 — the `#[sealed]` abstraction-barrier attribute parses onto a
//! `struct` (`.design/basis/06-provenance-and-sinks.md` REQ-8). A `#[sealed]`
//! `struct` sets `StructItem.sealed: true`, mirroring the `#[slag]`/`#[boundary]`
//! attribute precedent; an ordinary `struct` is `sealed: false`; and `#[sealed]`
//! is rejected anywhere but a `struct` (it is a clean-type-only barrier). Expected
//! shapes are hand-derived from REQ-8 (R-CHAR-3), never copied from parser output.

use thermite_syntax::{parse, Item};

#[test]
fn sealed_attribute_sets_the_flag_on_a_struct() {
    let r = parse("#[sealed] struct Sql { stmt: u64 }\n");
    assert!(r.is_clean(), "must parse clean, got {:?}", r.errors);
    let s = match &r.program.items[0] {
        Item::Struct(s) => s,
        other => panic!("item[0] must be Item::Struct, got {other:?}"),
    };
    assert_eq!(s.name, "Sql");
    assert!(
        s.sealed,
        "`#[sealed]` sets StructItem.sealed = true (REQ-8)"
    );
    // The seal does not disturb the field surface (REQ-1 unchanged).
    assert_eq!(s.fields.len(), 1);
    assert_eq!(s.fields[0].name, "stmt");
}

#[test]
fn a_plain_struct_is_not_sealed() {
    let r = parse("struct Tainted { raw: u64 }\n");
    assert!(r.is_clean(), "must parse clean, got {:?}", r.errors);
    let s = match &r.program.items[0] {
        Item::Struct(s) => s,
        other => panic!("item[0] must be Item::Struct, got {other:?}"),
    };
    assert!(
        !s.sealed,
        "a struct WITHOUT `#[sealed]` is sealed = false (REQ-8 — the seal is opt-in)"
    );
}

#[test]
fn sealed_with_an_inv_clause_still_parses() {
    // `#[sealed]` composes with the REQ-1 `inv` type-invariant clause.
    let r = parse("#[sealed] struct Cap { ok: bool } inv ok\n");
    assert!(r.is_clean(), "must parse clean, got {:?}", r.errors);
    let s = match &r.program.items[0] {
        Item::Struct(s) => s,
        other => panic!("item[0] must be Item::Struct, got {other:?}"),
    };
    assert!(s.sealed);
    assert!(
        s.inv.is_some(),
        "the `inv` clause is preserved under the seal"
    );
}

#[test]
fn sealed_on_an_enum_is_a_parse_error() {
    // `#[sealed]` is a struct-clean-type barrier (REQ-8); it does not attach to an
    // `enum`.
    let r = parse("#[sealed] enum Color { Red, Green }\n");
    assert!(
        !r.is_clean(),
        "`#[sealed] enum` must be a parse error (REQ-8: the seal is struct-only)"
    );
}

#[test]
fn sealed_on_a_fn_is_a_parse_error() {
    // A door is a `#[boundary]` fn, never `#[sealed]`; the seal is struct-only.
    let r = parse("#[sealed] fn f() -> u64 req true ens result == 0 fx pure { 0 }\n");
    assert!(
        !r.is_clean(),
        "`#[sealed] fn` must be a parse error (REQ-8: the seal is struct-only)"
    );
}
