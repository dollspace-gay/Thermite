use thermite_lower::{frozen_kernel_boundary_symbol, lower};

#[test]
fn canonical_kernel_boundary_lowers_to_exact_foreign_call() {
    let parsed = thermite_syntax::parse(
        r#"
#[frozen("kernel::atomic::cell@v1")] struct Atomic {}
enum Ordering { Relaxed, Acquire, Release, AcqRel, SeqCst }
#[boundary("kernel::atomic::load@v1")]
fn atomic_load(cell: &Atomic, order: Ordering) -> u64
  req !(order is Release) && !(order is AcqRel)
  ens result <= 18446744073709551615
  fx platform(atomic)
;
"#,
    );
    assert!(parsed.is_clean(), "fixture errors: {:?}", parsed.errors);
    let emitted = lower(&parsed.program).expect("lower canonical kernel boundary");
    assert!(emitted.contains("#[link_name = \"tpl_atomic_load\"]"));
    assert!(emitted.contains("fn __thermite_boundary_atomic_load("));
    assert!(emitted.contains("unsafe { __thermite_boundary_atomic_load(cell, order) }"));
    assert!(!emitted.contains("unimplemented!()"));
}

#[test]
fn frozen_symbol_mapping_rejects_noncanonical_names() {
    assert_eq!(
        frozen_kernel_boundary_symbol("kernel::atomic::compare_exchange@v1").as_deref(),
        Some("tpl_atomic_compare_exchange")
    );
    assert_eq!(
        frozen_kernel_boundary_symbol("kernel::atomic::load@v2"),
        None
    );
    assert_eq!(frozen_kernel_boundary_symbol("ext::atomic::load@v1"), None);
    assert_eq!(
        frozen_kernel_boundary_symbol("kernel::atomic::nested::load@v1"),
        None
    );
}
