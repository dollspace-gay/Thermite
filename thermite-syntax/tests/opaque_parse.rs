use thermite_syntax::{parse, Item};

#[test]
fn opaque_attribute_sets_only_the_opaque_flag() {
    let parsed = parse("#[opaque] struct Ledger { generation: u64 }\n");
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let Item::Struct(structure) = &parsed.program.items[0] else {
        panic!("expected a struct item");
    };
    assert!(structure.opaque);
    assert!(!structure.sealed);
}

#[test]
fn plain_and_sealed_struct_flags_remain_distinct() {
    let parsed =
        parse("struct Plain { value: u64 }\n#[sealed] struct Authority { identity: usize }\n");
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let Item::Struct(plain) = &parsed.program.items[0] else {
        panic!("expected the plain struct");
    };
    let Item::Struct(sealed) = &parsed.program.items[1] else {
        panic!("expected the sealed struct");
    };
    assert!(!plain.opaque && !plain.sealed);
    assert!(!sealed.opaque && sealed.sealed);
}

#[test]
fn opaque_is_struct_only() {
    for source in [
        "#[opaque] enum Outcome { Done }\n",
        "#[opaque] fn f() -> u64 req true ens result == 0 fx pure { 0 }\n",
        "#[opaque] spec fn f() -> bool dec 0 { true }\n",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.is_clean(),
            "`#[opaque]` must be struct-only: {source}"
        );
    }
}
