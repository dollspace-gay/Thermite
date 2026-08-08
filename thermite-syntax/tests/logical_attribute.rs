//! Parse coverage for the `#[logical(bound = …, observe = …)]` struct attribute
//! (`.design/build/aggregate-array-relations.md`, "Declaring a logical view").
//!
//! The attribute uses the `ident = "string"` field list `#[slag(...)]` already
//! uses and is dispatched by the same `parse_attribute` name switch that handles
//! `sealed`, `opaque`, and `boundary`. It combines with either construction
//! barrier, so a struct accepts an attribute list; every other item kind still
//! accepts at most one attribute. Field resolution belongs to `thermite-spec`,
//! so the parser records both names verbatim and rejects only the shapes the
//! grammar cannot represent.

use thermite_syntax::{parse, Item, StructItem};

fn struct_named<'a>(program: &'a thermite_syntax::Program, name: &str) -> &'a StructItem {
    program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(structure) if structure.name == name => Some(structure),
            _ => None,
        })
        .unwrap_or_else(|| panic!("program declares no struct `{name}`"))
}

#[test]
fn declares_the_index_space_and_its_observer() {
    let source = "\
const FIXED_RING_CAPACITY: usize = 64;

#[logical(bound = \"FIXED_RING_CAPACITY\", observe = \"fixed_ring_slot_spec\")]
struct FixedRing64 {
  slots: [u64; FIXED_RING_CAPACITY],
  head: usize,
}
";
    let parsed = parse(source);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let ring = struct_named(&parsed.program, "FixedRing64");
    let logical = ring
        .logical
        .as_ref()
        .expect("the declaration must carry its logical view");
    assert_eq!(logical.bound.as_deref(), Some("FIXED_RING_CAPACITY"));
    assert_eq!(logical.observe.as_deref(), Some("fixed_ring_slot_spec"));
    assert!(!ring.sealed && !ring.opaque);
}

#[test]
fn combines_with_the_opaque_construction_barrier() {
    // The design's own declaration example carries both attributes:
    // "#[logical(bound = \"FIXED_SLAB_CAPACITY\", observe = \"…\")]
    //  #[opaque] struct FixedSlab64 { … }".
    let source = "\
const FIXED_SLAB_CAPACITY: usize = 64;

#[logical(bound = \"FIXED_SLAB_CAPACITY\", observe = \"fixed_slab_slot_spec\")]
#[opaque] struct FixedSlab64 {
  slab_used: [bool; FIXED_SLAB_CAPACITY],
}
";
    let parsed = parse(source);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let slab = struct_named(&parsed.program, "FixedSlab64");
    assert!(slab.opaque);
    assert_eq!(
        slab.logical
            .as_ref()
            .and_then(|view| view.observe.as_deref()),
        Some("fixed_slab_slot_spec")
    );
}

#[test]
fn accepts_the_barrier_first_ordering() {
    let source = "\
#[opaque]
#[logical(bound = \"8\", observe = \"slot_spec\")]
struct State {
  slots: [u64; 8],
}
";
    let parsed = parse(source);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let state = struct_named(&parsed.program, "State");
    assert!(state.opaque);
    assert_eq!(
        state
            .logical
            .as_ref()
            .and_then(|view| view.bound.as_deref()),
        Some("8")
    );
}

#[test]
fn an_absent_field_reaches_the_validator_rather_than_the_parser() {
    // `bound`/`observe` resolution is `thermite-spec`'s admission rule, so the
    // parser records what it saw and leaves the diagnostic to the gate that
    // names the failing rule.
    let source = "\
#[logical(observe = \"slot_spec\")]
struct State {
  slots: [u64; 8],
}
";
    let parsed = parse(source);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let logical = struct_named(&parsed.program, "State")
        .logical
        .as_ref()
        .expect("the declaration is recorded even when a field is absent");
    assert_eq!(logical.bound, None);
    assert_eq!(logical.observe.as_deref(), Some("slot_spec"));
}

#[test]
fn a_struct_carries_one_logical_declaration() {
    let source = "\
#[logical(bound = \"8\", observe = \"first_spec\")]
#[logical(bound = \"8\", observe = \"second_spec\")]
struct State {
  slots: [u64; 8],
}
";
    let parsed = parse(source);
    assert!(
        !parsed.is_clean(),
        "a second `#[logical]` must be a duplicate error"
    );
}

#[test]
fn the_declaration_does_not_attach_to_a_function_or_enum() {
    for source in [
        "#[logical(bound = \"8\", observe = \"slot_spec\")]\nfn f(x: u64) -> u64\n  req true\n  ens result == x\n  fx pure\n{\n  x\n}\n",
        "#[logical(bound = \"8\", observe = \"slot_spec\")]\nenum E {\n  A,\n}\n",
        "#[logical(bound = \"8\", observe = \"slot_spec\")]\nspec fn s(x: u64) -> u64\n  dec x\n{\n  x\n}\n",
    ] {
        let parsed = parse(source);
        assert!(
            !parsed.is_clean(),
            "a declared index space is a struct property:\n{source}"
        );
    }
}

#[test]
fn one_construction_barrier_per_struct_still_holds() {
    let source = "\
#[sealed]
#[opaque] struct State {
  slots: [u64; 8],
}
";
    let parsed = parse(source);
    assert!(
        !parsed.is_clean(),
        "`#[sealed]` and `#[opaque]` remain mutually exclusive"
    );
}

#[test]
fn an_unknown_attribute_name_is_still_rejected() {
    let parsed = parse("#[quantified(bound = \"8\")]\nstruct State {\n  slots: [u64; 8],\n}\n");
    assert!(!parsed.is_clean(), "the attribute namespace stays closed");
}
