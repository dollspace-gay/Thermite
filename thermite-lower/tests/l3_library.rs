//! Structural pins for the export-aware library emitter used by the
//! correspondence-backed L3 artifact path.

use thermite_lower::{
    lower_l3_library, lower_l3_library_with_boundaries, L3BoundaryBinding, L3Export,
    L3ExportVisibility, L3LibraryTarget,
};

fn parse(source: &str) -> thermite_syntax::Program {
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    parsed.program
}

#[test]
fn hosted_library_has_only_explicit_public_exports_and_total_wrappers() {
    let program = parse(
        "fn helper(x: u64) -> u64 req true ens result == x fx pure { x } \
         fn direct(x: u64) -> u64 req true ens result == x fx pure { helper(x) } \
         fn guarded(x: u64) -> u64 req x < 100 ens result == x fx pure { x }",
    );
    let exports = [
        L3Export {
            source_name: "direct".to_string(),
            public_name: "direct".to_string(),
            wrapped: false,
            visibility: L3ExportVisibility::Public,
        },
        L3Export {
            source_name: "guarded".to_string(),
            public_name: "thermite_export_guarded_v1".to_string(),
            wrapped: true,
            visibility: L3ExportVisibility::Public,
        },
    ];
    let source = lower_l3_library(&program, &exports, L3LibraryTarget::Std).unwrap();

    assert!(source.starts_with("#![crate_type = \"rlib\"]\nuse vstd::prelude::*;"));
    assert!(source.contains("pub fn direct"));
    assert!(source.contains("\nfn helper"));
    assert!(!source.contains("pub fn helper"));
    assert!(source.contains("\nfn guarded"));
    assert!(!source.contains("pub fn guarded"));
    assert!(source.contains("pub fn thermite_export_guarded_v1"));
    assert!(source.contains("Result<u64, ThermiteContractError>"));
    assert!(source.contains("Err(ThermiteContractError::Precondition)"));
    assert!(!source.contains("fn main"));
    assert!(!source.contains("thermite_check!"));
    assert!(!source.contains("external_body"));
    assert!(!source.contains("lower_l1"));
}

#[test]
fn kernel_library_is_no_std_and_adds_alloc_only_when_needed() {
    let scalar = parse("fn id(x: u64) -> u64 req true ens result == x fx pure { x }");
    let scalar_export = [L3Export {
        source_name: "id".to_string(),
        public_name: "id".to_string(),
        wrapped: false,
        visibility: L3ExportVisibility::Public,
    }];
    let pure = lower_l3_library(&scalar, &scalar_export, L3LibraryTarget::Kernel).unwrap();
    assert!(pure.starts_with(
        "#![no_std]\n#![crate_type = \"rlib\"]\nuse verus_builtin::*;\nuse verus_builtin_macros::*;"
    ));
    assert!(!pure.contains("extern crate alloc"));
    assert!(!pure.contains("use vstd::"));
    assert!(!pure.contains("fn main"));

    let allocating = parse("fn keep(s: String) -> String req true ens result == s fx alloc { s }");
    let allocating_export = [L3Export {
        source_name: "keep".to_string(),
        public_name: "keep".to_string(),
        wrapped: false,
        visibility: L3ExportVisibility::Public,
    }];
    let with_alloc =
        lower_l3_library(&allocating, &allocating_export, L3LibraryTarget::Kernel).unwrap();
    assert!(with_alloc.contains("extern crate alloc;"));

    let bounded = parse(
        "fn keep(v: Vec<u64>) -> Vec<u64> req true ens result.len() == v.len() fx pure { v }",
    );
    let bounded_export = [L3Export {
        source_name: "keep".to_string(),
        public_name: "keep".to_string(),
        wrapped: false,
        visibility: L3ExportVisibility::Crate,
    }];
    let bounded_kernel =
        lower_l3_library(&bounded, &bounded_export, L3LibraryTarget::Kernel).unwrap();
    assert!(bounded_kernel.contains("pub struct TVecU64 { pub length: usize }"));
    assert!(bounded_kernel.contains("pub(crate) fn keep"));
    assert!(!bounded_kernel.contains("spec_get"));
    assert!(!bounded_kernel.contains("use vstd::"));
    assert!(!bounded_kernel.contains("extern crate alloc"));
}

#[test]
fn composition_library_delays_enum_items_past_randomized_verus_helper_synthesis() {
    let program = parse(
        "enum Action { Store { owner: u64, generation: u64, slot: u64, value: u64 }, Reject } \
         fn step(value: u64) -> Action req true ens match result { \
           Action::Store { owner, generation, slot, value: observed } => \
             owner == 7 && generation == 11 && slot == 0 && observed == value, \
           Action::Reject => false, \
         } fx pure { Action::Store { owner: 7, generation: 11, slot: 0, value: value } }",
    );
    let exports = [L3Export {
        source_name: "step".to_string(),
        public_name: "step".to_string(),
        wrapped: false,
        visibility: L3ExportVisibility::Crate,
    }];

    let source = lower_l3_library(&program, &exports, L3LibraryTarget::Kernel).unwrap();
    assert!(source.contains("macro_rules! __thermite_deterministic_enum"));
    assert!(source.contains("#[verus::internal(verus_macro)]"));
    assert!(source.contains("__thermite_deterministic_enum! {\npub enum Action"));
    assert!(source.contains("Store { owner: u64, generation: u64, slot: u64, value: u64 }"));

    let public_exports = [L3Export {
        visibility: L3ExportVisibility::Public,
        ..exports[0].clone()
    }];
    let ordinary = lower_l3_library(&program, &public_exports, L3LibraryTarget::Kernel).unwrap();
    assert!(!ordinary.contains("__thermite_deterministic_enum"));
    assert!(ordinary.contains("pub enum Action"));
}

#[test]
fn registry_bound_boundary_is_a_checked_exact_target_call() {
    let program = parse(
        "#[boundary(\"platform::read\")] \
         fn read(x: u64) -> u64 req true ens result == x fx platform(clock); \
         fn observe(x: u64) -> u64 req true ens result == x fx platform(clock) { read(x) }",
    );
    let exports = [L3Export {
        source_name: "observe".to_string(),
        public_name: "observe".to_string(),
        wrapped: false,
        visibility: L3ExportVisibility::Crate,
    }];
    let bindings = [L3BoundaryBinding {
        source_name: "read".to_string(),
        call_target: "platform_shell::read_impl".to_string(),
    }];

    let source =
        lower_l3_library_with_boundaries(&program, &exports, L3LibraryTarget::Kernel, &bindings)
            .unwrap();
    assert!(source.contains("fn read(x: u64) -> (result: u64)"));
    assert!(source.contains("platform_shell::read_impl(x)"));
    assert!(!source.contains("external_body"));
    assert!(!source.contains("unimplemented!"));

    let ordinary = lower_l3_library(&program, &exports, L3LibraryTarget::Kernel).unwrap();
    assert!(ordinary.contains("#[verifier::external_body]"));
}
