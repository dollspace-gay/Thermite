use thermite_syntax::Item;

#[test]
fn function_context_entry_is_the_exact_body_used_by_full_lowering() {
    let source = r#"
struct State { cursor: u64, total: u64 }
fn count(limit: u64) -> State
  req limit <= 10
  ens result.cursor == limit
  ens result.total == limit
  fx pure
{
  let mut state: State = State { cursor: 0, total: 0 };
  while state.cursor < limit
    inv state.cursor <= limit
    inv state.total == state.cursor
    dec limit - state.cursor
  {
    state.total = state.cursor + 1;
    state.cursor = state.cursor + 1;
  }
  state
}
"#;
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let function = parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) => Some(function),
            _ => None,
        })
        .expect("count function");
    let body = thermite_lower::lower_exec_body_in_function(&parsed.program, function)
        .expect("function-context body lowers");
    let complete = thermite_lower::lower(&parsed.program).expect("complete program lowers");
    assert!(body.contains("while state.cursor < limit"), "{body}");
    assert!(body.contains("invariant\n"), "{body}");
    assert!(body.contains("decreases limit - state.cursor"), "{body}");
    assert!(
        complete.contains(&format!("{{\n{body}}}\n")),
        "standalone function-context body diverged from full production lowering\n--- body ---\n{body}\n--- complete ---\n{complete}"
    );
}
