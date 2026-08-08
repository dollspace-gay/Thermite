use thermite_spec::{validate, SpecError};
use thermite_syntax::parse;

fn validate_src(src: &str) -> Result<(), Vec<SpecError>> {
    let parsed = parse(src);
    assert!(
        parsed.is_clean(),
        "fixture parse errors: {:?}",
        parsed.errors
    );
    validate(&parsed.program)
}

#[test]
fn accepts_total_u64_bit_methods_in_contracts_and_bodies() {
    let source = r#"
fn update(word: u64, bit: usize) -> u64
  req true
  ens result == word.bit_set(bit)
  ens bit >= 64 || result.bit_test(bit)
  fx pure
{
  word.bit_set(bit)
}

fn clear(word: u64, bit: usize) -> u64
  req true
  ens result == word.bit_clear(bit)
  ens bit >= 64 || !result.bit_test(bit)
  fx pure
{
  word.bit_clear(bit)
}

fn preserve(word: u64, changed: usize, observed: usize) -> bool
  req true
  ens result == word.bit_set_preserves_other(changed, observed)
    && result == word.bit_clear_preserves_other(changed, observed)
  fx pure
{
  word.bit_set_preserves_other(changed, observed)
    && word.bit_clear_preserves_other(changed, observed)
}
"#;
    assert!(validate_src(source).is_ok());
}

#[test]
fn rejects_wrong_u64_bit_method_arity() {
    for source in [
        "fn bad(word: u64) -> bool req true ens true fx pure { word.bit_test() }",
        "fn bad(word: u64, bit: usize) -> u64 req true ens result == word.bit_set(bit, bit) fx pure { word }",
        "fn bad(word: u64, bit: usize) -> u64 req true ens true fx pure { word.bit_clear(bit, bit) }",
    ] {
        let errors = validate_src(source).expect_err("wrong bit-method arity must fail");
        assert!(errors.iter().any(|error| matches!(
            error,
            SpecError::ForbiddenCall { detail, .. }
                if detail.contains("expects exactly one bit-index argument")
        )));
    }

    for source in [
        "fn bad(word: u64, bit: usize) -> bool req true ens true fx pure { word.bit_set_preserves_other(bit) }",
        "fn bad(word: u64, bit: usize) -> bool req true ens true fx pure { word.bit_clear_preserves_other(bit, bit, bit) }",
    ] {
        let errors = validate_src(source).expect_err("wrong preservation-method arity must fail");
        assert!(errors.iter().any(|error| matches!(
            error,
            SpecError::ForbiddenCall { detail, .. }
                if detail.contains("expects exactly 2 bit-index argument")
        )));
    }
}
