use thermite_lower::{lower, lower_l1, lower_l2};
use thermite_syntax::parse;

const SOURCE: &str = r#"
fn bit_roundtrip(word: u64, bit: usize) -> bool
  req bit < 64
  ens result
  fx pure
{
  let set: u64 = word.bit_set(bit);
  let cleared: u64 = set.bit_clear(bit);
  set.bit_test(bit) && !cleared.bit_test(bit)
}

fn bit_oob(word: u64, bit: usize) -> bool
  req bit >= 64
  ens result
  fx pure
{
  !word.bit_test(bit)
    && word.bit_set(bit) == word
    && word.bit_clear(bit) == word
}

fn bit_other(word: u64, changed: usize, observed: usize) -> bool
  req changed < 64 && observed < 64 && changed != observed
  ens result
  fx pure
{
  word.bit_set_preserves_other(changed, observed)
    && word.bit_clear_preserves_other(changed, observed)
}
"#;

fn program() -> thermite_syntax::Program {
    let parsed = parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("u64 bit-method program must validate");
    parsed.program
}

#[test]
fn l3_emits_the_finite_directly_verified_bit_bridge() {
    let emitted = lower(&program()).expect("L3 u64 bit lowering must succeed");
    assert!(
        emitted.contains("spec fn __thermite_u64_bit_mask"),
        "{emitted}"
    );
    assert!(
        emitted.contains("63 => 9223372036854775808u64"),
        "{emitted}"
    );
    assert!(emitted.contains("by(bit_vector)"), "{emitted}");
    assert!(
        emitted.contains("__thermite_u64_bit_set(word, bit)"),
        "{emitted}"
    );
    assert!(
        emitted.contains("__thermite_u64_bit_clear(set, bit)"),
        "{emitted}"
    );
    assert!(
        emitted.contains("__thermite_u64_bit_test(set, bit)"),
        "{emitted}"
    );
    assert!(
        emitted.contains("fn __thermite_u64_bit_set_preserves_other"),
        "{emitted}"
    );
    assert!(
        emitted.contains("fn __thermite_u64_bit_clear_preserves_other"),
        "{emitted}"
    );
    assert!(
        emitted.contains("__thermite_u64_bit_mask_shift_lemma"),
        "{emitted}"
    );
    assert!(
        emitted.contains("requires\n                changed64 < 64u64"),
        "{emitted}"
    );
}

#[test]
fn runtime_and_bounded_backends_keep_total_out_of_range_semantics() {
    let program = program();
    let l1 = lower_l1(&program).expect("L1 u64 bit lowering must succeed");
    assert!(l1.contains("< 64usize"), "{l1}");
    assert!(l1.contains("1u64 <<"), "{l1}");
    assert!(l1.contains("else { false }"), "{l1}");
    assert!(l1.contains("else { word }"), "{l1}");
    assert!(l1.contains("__thermite_changed_mask"), "{l1}");
    assert!(l1.contains("__thermite_observed_mask"), "{l1}");

    let l2 = lower_l2(&program).expect("L2 u64 bit lowering must succeed");
    assert!(l2.contains("< 64usize"), "{l2}");
    assert!(l2.contains("1u64 <<"), "{l2}");
    assert!(l2.contains("else { false }"), "{l2}");
    assert!(l2.contains("__thermite_changed_mask"), "{l2}");
}
