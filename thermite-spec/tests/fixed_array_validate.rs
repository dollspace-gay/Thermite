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

#[test]
fn repeat_initialization_requires_a_copy_safe_element_type() {
    let owned_string = validate_src(
        "fn bad(value: String) -> [String; 2] req true ens true fx pure { [value; 2] }",
    )
    .expect_err("repeat initialization must not clone an owned String implicitly");
    assert!(owned_string
        .iter()
        .any(|error| matches!(error, SpecError::ArrayRepeatRequiresCopy { .. })));

    let mutable_alias = validate_src(
        "fn bad(value: &mut u64) -> [&mut u64; 2] req true ens true fx pure { [value; 2] }",
    )
    .expect_err("repeat initialization must not duplicate a mutable reference");
    assert!(mutable_alias
        .iter()
        .any(|error| matches!(error, SpecError::ArrayRepeatRequiresCopy { .. })));

    let nested_mismatch =
        validate_src("fn bad() -> [[u64; 2]; 2] req true ens true fx pure { [[0; 3]; 2] }")
            .expect_err("nested initializer lengths must be checked recursively");
    assert!(nested_mismatch.iter().any(|error| matches!(
        error,
        SpecError::ArrayLengthMismatch {
            expected: 2,
            found: 3,
            ..
        }
    )));

    assert!(validate_src(
        "fn good(pair: (u64, bool)) -> [(u64, bool); 2] req true ens true fx pure { [pair; 2] }"
    )
    .is_ok());
}

#[test]
fn array_equality_accepts_matching_structural_arrays() {
    assert!(validate_src(
        "fn equal(left: [u64; 4], right: [u64; 4]) -> bool\n\
         req true\n\
         ens result == left.array_eq(right)\n\
         fx pure\n\
         { left.array_eq(right) }"
    )
    .is_ok());

    assert!(validate_src(
        "fn nested(left: [[u64; 2]; 2], right: [[u64; 2]; 2]) -> bool\n\
         req true ens result == left.array_eq(right) fx pure { left.array_eq(right) }",
    )
    .is_ok());

    assert!(validate_src(
        "struct Stamp { words: [u64; 2], flags: (bool, u8) }\n\
         struct Slot { stamp: Stamp, owner: usize }\n\
         fn equal(left: [Slot; 4], right: [Slot; 4]) -> bool\n\
         req true ens result == left.array_eq(right) fx pure { left.array_eq(right) }",
    )
    .is_ok());

    let capacity_mismatch = validate_src(
        "fn bad(left: [u64; 2], right: [u64; 3]) -> bool\n\
         req true ens true fx pure { left.array_eq(right) }",
    )
    .expect_err("different array types cannot use the primitive equality helper");
    assert!(capacity_mismatch.iter().any(|error| matches!(
        error,
        SpecError::ArrayEqualityRequiresStructuralArrays { .. }
    )));

    let scalar = validate_src(
        "fn bad(left: u64, right: u64) -> bool\n\
         req true ens true fx pure { left.array_eq(right) }",
    )
    .expect_err("the reserved method must not dispatch on a non-array receiver");
    assert!(scalar.iter().any(|error| matches!(
        error,
        SpecError::ArrayEqualityRequiresStructuralArrays { .. }
    )));
}

#[test]
fn array_equality_rejects_hidden_recursive_and_heap_backed_records() {
    for (label, source) in [
        (
            "sealed authority",
            "#[sealed] struct Token { raw: u64 }\n\
             fn bad(left: [Token; 2], right: [Token; 2]) -> bool\n\
             req true ens true fx pure { left.array_eq(right) }",
        ),
        (
            "opaque representation",
            "#[opaque] struct State { raw: u64 }\n\
             fn bad(left: [State; 2], right: [State; 2]) -> bool\n\
             req true ens true fx pure { left.array_eq(right) }",
        ),
        (
            "recursive record",
            "struct Node { next: Box<Node> }\n\
             fn bad(left: [Node; 2], right: [Node; 2]) -> bool\n\
             req true ens true fx pure { left.array_eq(right) }",
        ),
        (
            "heap-backed field",
            "struct Label { text: String }\n\
             fn bad(left: [Label; 2], right: [Label; 2]) -> bool\n\
             req true ens true fx pure { left.array_eq(right) }",
        ),
        (
            "enum element",
            "enum State { Idle, Busy(u64) }\n\
             fn bad(left: [State; 2], right: [State; 2]) -> bool\n\
             req true ens true fx pure { left.array_eq(right) }",
        ),
    ] {
        let errors = validate_src(source).expect_err(label);
        assert!(
            errors.iter().any(|error| matches!(
                error,
                SpecError::ArrayEqualityRequiresStructuralArrays { .. }
            )),
            "{label}: {errors:?}"
        );
    }
}

#[test]
fn array_same_except_requires_matching_structural_arrays() {
    assert!(validate_src(
        "fn same_except(left: [u64; 4], right: [u64; 4], at: usize) -> bool\n\
         req true\n\
         ens result == left.array_same_except(right, at)\n\
         fx pure\n\
         { left.array_same_except(right, at) }"
    )
    .is_ok());

    let capacity_mismatch = validate_src(
        "fn bad(left: [u64; 2], right: [u64; 3], at: usize) -> bool\n\
         req true ens true fx pure { left.array_same_except(right, at) }",
    )
    .expect_err("the frame relation requires equal array types");
    assert!(capacity_mismatch.iter().any(|error| matches!(
        error,
        SpecError::ArrayEqualityRequiresStructuralArrays { .. }
    )));

    let scalar = validate_src(
        "fn bad(left: u64, right: u64, at: usize) -> bool\n\
         req true ens true fx pure { left.array_same_except(right, at) }",
    )
    .expect_err("the frame relation must reject scalar receivers");
    assert!(scalar.iter().any(|error| matches!(
        error,
        SpecError::ArrayEqualityRequiresStructuralArrays { .. }
    )));

    assert!(validate_src(
        "fn same_except_two(left: [u64; 4], right: [u64; 4], first: usize, second: usize) -> bool\n\
         req true\n\
         ens result == left.array_same_except_two(right, first, second)\n\
         fx pure\n\
         { left.array_same_except_two(right, first, second) }"
    )
    .is_ok());

    for source in [
        "fn bad(left: [u64; 2], right: [u64; 3], first: usize, second: usize) -> bool\n\
         req true ens true fx pure { left.array_same_except_two(right, first, second) }",
        "fn bad(left: [u64; 2], right: [u64; 2], first: usize) -> bool\n\
         req true ens true fx pure { left.array_same_except_two(right, first) }",
    ] {
        let errors = validate_src(source).expect_err("two-index framing must fail closed");
        assert!(errors.iter().any(|error| matches!(
            error,
            SpecError::ArrayEqualityRequiresStructuralArrays { .. }
        )));
    }
}
