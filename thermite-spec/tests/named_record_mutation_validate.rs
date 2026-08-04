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

fn mutation_error(src: &str) -> Vec<SpecError> {
    validate_src(src).expect_err("invalid named-record mutation must fail before lowering")
}

#[test]
fn accepts_exclusive_plain_and_opaque_record_mutation() {
    for source in [
        r#"
struct State { generation: u64, occupied: bool }
fn advance(state: &mut State, next: u64) -> bool
  req next > old(state).generation
  ens result == old(state).occupied
  ens final(state).generation == next
  ens final(state).occupied == old(state).occupied
  fx pure
{
  let previous: bool = state.occupied;
  state.generation = next;
  previous
}
"#,
        r#"
#[opaque] struct State { generation: u64, occupied: bool }
fn set(state: &mut State, next: u64) -> ()
  req true
  ens final(state).generation == next
  ens final(state).occupied == old(state).occupied
  fx pure
{
  state.generation = next;
}
"#,
        r#"
struct State { generation: u64 }
fn replace(next: u64) -> State
  req true
  ens result.generation == next
  fx pure
{
  let mut state: State = State { generation: 0 };
  state.generation = next;
  state
}
"#,
    ] {
        assert!(validate_src(source).is_ok(), "{source}");
    }
}

#[test]
fn rejects_shared_and_immutable_record_roots() {
    for (label, source) in [
        (
            "shared borrow",
            "struct State { value: u64 }\n\
             fn bad(state: &State) -> u64 req true ens true fx pure {\n\
               state.value = 1; state.value\n\
             }",
        ),
        (
            "owned immutable parameter",
            "struct State { value: u64 }\n\
             fn bad(state: State) -> u64 req true ens true fx pure {\n\
               state.value = 1; state.value\n\
             }",
        ),
        (
            "immutable local",
            "struct State { value: u64 }\n\
             fn bad() -> State req true ens true fx pure {\n\
               let state: State = State { value: 0 };\n\
               state.value = 1; state\n\
             }",
        ),
    ] {
        let errors = mutation_error(source);
        assert!(
            errors.iter().any(|error| matches!(
                error,
                SpecError::InvalidNamedRecordMutation { detail, .. }
                    if detail.contains("not writable")
            )),
            "{label}: {errors:?}"
        );
    }
}

#[test]
fn rejects_sealed_nested_recursive_heap_and_wrong_field_targets() {
    for (label, source, needle) in [
        (
            "sealed root",
            "#[sealed] struct Token { raw: u64 }\n\
             fn bad(token: &mut Token) -> u64 req true ens true fx pure {\n\
               token.raw = 1; token.raw\n\
             }",
            "sealed",
        ),
        (
            "nested target",
            "struct Inner { value: u64 } struct Outer { inner: Inner }\n\
             fn bad(state: &mut Outer) -> u64 req true ens true fx pure {\n\
               state.inner.value = 1; state.inner.value\n\
             }",
            "exactly `root.field`",
        ),
        (
            "recursive root",
            "struct Node { next: Box<Node> }\n\
             fn bad(state: &mut Node, next: Box<Node>) -> u64 req true ens true fx pure {\n\
               state.next = next; 0\n\
             }",
            "recursive",
        ),
        (
            "heap field",
            "struct State { text: String }\n\
             fn bad(state: &mut State, text: String) -> u64 req true ens true fx pure {\n\
               state.text = text; 0\n\
             }",
            "heap-backed",
        ),
        (
            "field from another record",
            "struct Left { left: u64 } struct Right { right: u64 }\n\
             fn bad(state: &mut Left) -> u64 req true ens true fx pure {\n\
               state.right = 1; state.left\n\
             }",
            "exact record type",
        ),
    ] {
        let errors = mutation_error(source);
        assert!(
            errors.iter().any(|error| matches!(
                error,
                SpecError::InvalidNamedRecordMutation { detail, .. }
                    if detail.contains(needle)
            )),
            "{label}: {errors:?}"
        );
    }
}

#[test]
fn rejects_untyped_record_root() {
    for (label, source) in [
        (
            "untyped local",
            "struct State { value: u64 }\n\
             fn bad() -> u64 req true ens true fx pure {\n\
               let mut state = State { value: 0 };\n\
               state.value = 1; state.value\n\
             }",
        ),
        (
            "branch-local binding",
            "struct State { value: u64 }\n\
             fn bad(choose: bool) -> u64 req true ens true fx pure {\n\
               if choose { let mut state: State = State { value: 0 }; }\n\
               state.value = 1; 0\n\
             }",
        ),
    ] {
        let errors = mutation_error(source);
        assert!(
            errors.iter().any(|error| matches!(
                error,
                SpecError::InvalidNamedRecordMutation { detail, .. }
                    if detail.contains("no declared source type")
            )),
            "{label}: {errors:?}"
        );
    }
}
