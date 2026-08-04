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
fn accepts_bounded_named_and_nested_fixed_arrays() {
    let source = r#"
const ROWS: usize = 4;
const COLS: usize = 8;

struct Table { cells: [[u64; COLS]; ROWS] }

fn initialize(value: u64) -> [u64; COLS]
  req true
  ens result.len() == COLS
  fx pure
{
  let row: [u64; COLS] = [value; COLS];
  row
}
"#;
    assert!(validate_src(source).is_ok());
}

#[test]
fn rejects_unknown_duplicate_and_oversized_capacities() {
    let unknown =
        validate_src("fn bad() -> [u64; MISSING] req true ens true fx pure { [0; MISSING] }")
            .expect_err("unknown capacity must fail before lowering");
    assert!(unknown.iter().any(
        |error| matches!(error, SpecError::UnknownArrayCapacity { name, .. } if name == "MISSING")
    ));

    let duplicate = validate_src(
        "const CAP: usize = 2; const CAP: usize = 3;\n\
         fn bad() -> [u64; CAP] req true ens true fx pure { [0; CAP] }",
    )
    .expect_err("duplicate capacity must fail before lowering");
    assert!(duplicate.iter().any(
        |error| matches!(error, SpecError::DuplicateArrayCapacity { name, .. } if name == "CAP")
    ));

    let oversized = validate_src(
        "const HUGE: usize = 1048577;\n\
         fn bad() -> [u8; HUGE] req true ens true fx pure { [0; HUGE] }",
    )
    .expect_err("oversized capacity must fail before lowering");
    assert!(oversized.iter().any(|error| matches!(
        error,
        SpecError::ArrayCapacityTooLarge {
            value: 1_048_577,
            ..
        }
    )));
}

#[test]
fn rejects_expanded_nested_size_and_annotated_initializer_mismatch() {
    let expanded = validate_src(
        "fn bad() -> [[u8; 1024]; 1025] req true ens true fx pure { [[0; 1024]; 1025] }",
    )
    .expect_err("recursively expanded array must be bounded");
    assert!(expanded.iter().any(|error| matches!(
        error,
        SpecError::ArrayExpandedSizeTooLarge {
            elements: 1_049_600,
            ..
        }
    )));

    let mismatch = validate_src(
        "fn bad() -> [u8; 4] req true ens true fx pure {\n\
           let values: [u8; 4] = [1, 2, 3];\n\
           values\n\
         }",
    )
    .expect_err("annotated exact initializer length must match");
    assert!(mismatch.iter().any(|error| matches!(
        error,
        SpecError::ArrayLengthMismatch {
            expected: 4,
            found: 3,
            ..
        }
    )));
}
