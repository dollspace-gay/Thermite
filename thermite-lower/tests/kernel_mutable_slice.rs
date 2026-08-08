use thermite_syntax::parse;

const SOURCE: &str = "fn write_byte(data: &mut [u8], at: usize, value: u8) -> u8\n\
req at < data.len()\n\
ens result == value\n\
ens final(data)[at] == value\n\
fx platform(memory)\n\
{ data[at] = value; value }\n";

#[test]
fn mutable_slice_write_lowers_with_verus_final_view() {
    let parsed = parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("final is a closed state-view primitive");
    let lowered = thermite_lower::lower(&parsed.program).expect("mutable slice must lower");
    assert!(lowered.contains("data: &mut [u8]"), "{lowered}");
    assert!(
        lowered.contains("final(data)@[at as int] == value"),
        "{lowered}"
    );
    assert!(lowered.contains("data[at] = value;"), "{lowered}");
}

#[test]
fn aggregate_slice_and_fixed_array_borrows_keep_exact_native_types() {
    let source = "fn write_row(data: &mut [[u64; 2]], at: usize, value: u64) -> u64\n\
req at < data.len()\n\
ens result == value\n\
ens final(data)[at][0] == value\n\
fx platform(memory)\n\
{ data[at] = [value, value]; value }\n\
fn write_array(data: &mut [u64; 4], at: usize, value: u64) -> u64\n\
req at < 4\n\
ens result == value\n\
ens final(data)[at] == value\n\
fx platform(memory)\n\
{ data[at] = value; value }\n";
    let parsed = parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("aggregate storage must validate");
    let lowered = thermite_lower::lower(&parsed.program).expect("aggregate storage must lower");
    assert!(lowered.contains("data: &mut [[u64; 2]]"), "{lowered}");
    assert!(lowered.contains("data: &mut [u64; 4]"), "{lowered}");
    assert!(
        lowered.contains("final(data)@[at as int]@[0 as int] == value"),
        "{lowered}"
    );
}
