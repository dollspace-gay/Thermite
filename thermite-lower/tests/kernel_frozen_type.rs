use thermite_lower::{L3Export, L3ExportVisibility, L3LibraryTarget};
use thermite_syntax::parse;

#[test]
fn frozen_atomic_type_lowers_to_exact_verified_platform_type() {
    let parsed = parse(
        "#[frozen(\"kernel::atomic::cell@v1\")] struct Atomic {}\n\
         fn use_atomic(cell: &Atomic) -> u64 req true ens result == 0 fx pure { 0 }\n",
    );
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("frozen type validates");
    let lowered = thermite_lower::lower_l3_library(
        &parsed.program,
        &[L3Export {
            source_name: "use_atomic".to_string(),
            public_name: "use_atomic".to_string(),
            wrapped: false,
            visibility: L3ExportVisibility::Crate,
        }],
        L3LibraryTarget::Kernel,
    )
    .expect("frozen type lowers");
    assert!(
        lowered.contains("pub type Atomic = atomic::ExactAtomicU64;"),
        "{lowered}"
    );
    assert!(!lowered.contains("pub struct Atomic"), "{lowered}");
}

#[test]
fn unknown_or_stateful_frozen_types_fail_closed() {
    for source in [
        "#[frozen(\"kernel::atomic::unknown@v1\")] struct Atomic {}\n",
        "#[frozen(\"kernel::atomic::cell@v1\")] struct Atomic { slot: u64 }\n",
    ] {
        let parsed = parse(source);
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        assert!(thermite_lower::lower(&parsed.program).is_err(), "{source}");
    }
}
