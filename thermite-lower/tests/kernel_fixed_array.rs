use thermite_syntax::parse;

const SOURCE: &str = r#"
fn fixed_roundtrip(at: usize, value: u64) -> u64
  req at < 8
  ens result == value
  fx pure
{
  let mut slots: FixedArray8<u64> = FixedArray8::fill(0);
  slots.set(at, value);
  slots.get(at)
}
"#;

#[test]
fn fixed_array8_lowers_to_allocation_free_verified_storage() {
    let parsed = parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("fixed storage program validates");
    let lowered = thermite_lower::lower(&parsed.program).expect("fixed storage lowers");

    assert!(lowered.contains("pub struct TFixedArray8U64"), "{lowered}");
    for index in 0..8 {
        assert!(
            lowered.contains(&format!("pub slot{index}: u64")),
            "missing fixed slot {index}:\n{lowered}"
        );
    }
    assert!(
        lowered.contains("pub fn fill(value: u64) -> (result: TFixedArray8U64)"),
        "{lowered}"
    );
    assert!(
        lowered.contains("final(self).spec_get(i as int) == value"),
        "{lowered}"
    );
    assert!(
        lowered.contains("let mut slots: TFixedArray8U64 = TFixedArray8U64::fill(0);"),
        "{lowered}"
    );
    assert!(
        !lowered.contains("Vec<") && !lowered.contains("Vec::new"),
        "fixed storage must not acquire a heap-backed representation:\n{lowered}"
    );
}
