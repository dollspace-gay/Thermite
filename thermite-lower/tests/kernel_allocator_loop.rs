use thermite_syntax::parse;

const SOURCE: &str = include_str!("../../conformance/kernel/memory.th");
const ATOMIC_SOURCE: &str = include_str!("../../conformance/kernel/atomic.th");
const ALLOCATOR_SOURCE: &str = include_str!("../../conformance/kernel/allocator.th");
const RUNTIME_SOURCE: &str =
    include_str!("../../platform/x86_64-pc-uefi-smp-v1/runtime/src/post_firmware.rs");

#[test]
fn kernel_allocator_first_fit_keeps_recursive_spec_and_bounded_exec_loop() {
    let parsed = parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("kernel memory policy validates");
    let lowered = thermite_lower::lower(&parsed.program).expect("kernel memory policy lowers");

    assert!(
        lowered.contains("pub open spec fn allocator_first_available_spec"),
        "{lowered}"
    );
    assert!(
        lowered.contains("decreases if first < 64 { 64 - first } else { 0 }"),
        "{lowered}"
    );
    assert!(
        lowered.contains("while first < 64 && first <= 64 - pages"),
        "{lowered}"
    );
    assert!(
        lowered.contains("allocator_first_available_spec(observed, pages, (first + 1) as u64)"),
        "{lowered}"
    );
}

#[test]
fn kernel_allocator_claim_loop_is_generated_and_runtime_does_not_duplicate_it() {
    let source = format!("{ATOMIC_SOURCE}\n{SOURCE}\n{ALLOCATOR_SOURCE}\n");
    let parsed = parse(&source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("atomic allocator policy validates");
    let lowered = thermite_lower::lower(&parsed.program).expect("atomic allocator policy lowers");

    assert!(lowered.contains("fn allocator_claim_first"), "{lowered}");
    assert!(
        lowered.contains("while attempts < 64 && first == 64"),
        "{lowered}"
    );
    assert!(
        lowered.contains("atomic_boundary_load(cell, Ordering::SeqCst)"),
        "{lowered}"
    );
    assert!(
        lowered.contains("atomic_compare_exchange(cell, observed, claimed)"),
        "{lowered}"
    );
    assert!(
        RUNTIME_SOURCE.contains("allocator_claim_first(&HEAP_BITMAP, pages as u64)"),
        "the live allocator must call the generated claim state machine"
    );
    assert!(
        !RUNTIME_SOURCE.contains("let observed = exact_load(&HEAP_BITMAP)"),
        "runtime Rust must not retain a parallel allocation-claim loop"
    );
}
