use thermite_syntax::parse;

const SOURCE: &str = r#"
spec fn selected(xs: &[u32], at: usize) -> u64
  dec xs.len()
{
  if at < xs.len() {
    xs[at] as u64
  } else {
    0
  }
}

fn read_selected(xs: &[u32], at: usize) -> u64
  req at < xs.len()
  ens result == selected(xs, at)
  fx pure
{
  xs[at] as u64
}
"#;

#[test]
fn spec_slice_index_uses_seq_directly_while_contract_uses_runtime_view() {
    let parsed = parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("spec slice program validates");
    let lowered = thermite_lower::lower(&parsed.program).expect("spec slice index must lower");

    assert!(
        lowered.contains("spec fn selected(xs: Seq<u32>, at: usize)"),
        "{lowered}"
    );
    assert!(lowered.contains("xs[at as int] as u64"), "{lowered}");
    assert!(!lowered.contains("xs@[at as int] as u64"), "{lowered}");
    assert!(lowered.contains("result == selected(xs@, at)"), "{lowered}");
}
