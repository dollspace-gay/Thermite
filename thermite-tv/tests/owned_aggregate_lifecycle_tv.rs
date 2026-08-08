use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::Item;
use thermite_tv::obligation::{
    body_equivalence_obligation, exec_equivalence_obligation, BodyObligationFrame, BodyParamDecl,
    ExecObligationFrame, ExecParamDecl,
};
use thermite_tv::{MutableRecordFrame, NamedRecordFrame, RecordFieldFrame};

const SOURCE: &str = r#"
struct State { first: u64, second: u64, occupied: bool }
fn transition(state: State, next: u64) -> State
  req true
  ens result.first == next
  ens result.second == next
  ens result.occupied == state.occupied
  fx pure
{
  let mut updated: State = state;
  updated.first = next;
  updated.second = updated.first;
  updated
}
"#;

const ARRAY_SOURCE: &str = r#"
const SLOTS: usize = 2;
struct Store { slots: [u64; SLOTS], tag: u64 }
fn replace_slot(state: Store, index: usize, value: u64) -> Store
  req index < SLOTS
  ens result.slots[index] == value
  ens result.tag == state.tag
  fx pure
{
  let mut slots: [u64; SLOTS] = state.slots;
  slots[index] = value;
  let mut updated: Store = state;
  updated.slots = slots;
  updated
}
"#;

const NESTED_SOURCE: &str = r#"
const SLOTS: usize = 2;
struct Inner { value: u64, guard: u64 }
struct Nested { inner: Inner, slots: [u64; SLOTS], tag: u64 }
fn nested_owned(state: Nested, index: usize, next: u64) -> Nested
  req index < SLOTS && next < 1000
  ens result.inner.value == next
  ens result.inner.guard == state.inner.guard
  ens result.slots[index] == next + 1
  ens result.tag == state.tag
  fx pure
{
  let mut updated: Nested = state;
  updated.inner.value = next;
  updated.slots[index] = updated.inner.value + 1;
  updated
}
fn nested_borrowed(state: &mut Nested, index: usize, next: u64) -> u64
  req index < SLOTS && next < 1000
  ens result == next
  ens final(state).inner.value == next
  ens final(state).inner.guard == old(state).inner.guard
  ens final(state).slots[index] == next + 1
  ens final(state).tag == old(state).tag
  fx pure
{
  state.inner.value = next;
  state.slots[index] = state.inner.value + 1;
  state.inner.value
}
"#;

fn function(source: &str, index: usize) -> thermite_syntax::FnItem {
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[index] else {
        panic!("expected function at item {index}");
    };
    function.clone()
}

fn state_frame() -> BodyObligationFrame {
    let record = NamedRecordFrame::new(
        "State",
        vec![
            RecordFieldFrame::new("first", false),
            RecordFieldFrame::new("second", false),
            RecordFieldFrame::new("occupied", false),
        ],
    );
    BodyObligationFrame {
        spec_defs: vec![
            "pub struct State {\n    pub first: u64,\n    pub second: u64,\n    pub occupied: bool,\n}"
                .to_string(),
        ],
        params: vec![
            BodyParamDecl::new("state", "State"),
            BodyParamDecl::new("next", "u64"),
        ],
        ret_type: "State".to_string(),
        named_records: vec![record.clone()],
        result_record: Some(record),
        ..Default::default()
    }
}

fn store_frame() -> BodyObligationFrame {
    let record = NamedRecordFrame::new(
        "Store",
        vec![
            RecordFieldFrame::new("slots", true),
            RecordFieldFrame::new("tag", false),
        ],
    );
    BodyObligationFrame {
        spec_defs: vec![
            "pub const SLOTS: usize = 2;".to_string(),
            "pub struct Store {\n    pub slots: [u64; SLOTS],\n    pub tag: u64,\n}".to_string(),
        ],
        params: vec![
            BodyParamDecl::new("state", "Store"),
            BodyParamDecl::new("index", "usize"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "Store".to_string(),
        req: Some("index < SLOTS".to_string()),
        fixed_array_fields: vec!["state.slots".to_string()],
        named_records: vec![record.clone()],
        result_record: Some(record),
        ..Default::default()
    }
}

fn parsed_record_frame(source: &str, type_name: &str) -> NamedRecordFrame {
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let record = parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(record) if record.name == type_name => Some(record),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing record `{type_name}`"));
    NamedRecordFrame::new(
        type_name,
        record
            .fields
            .iter()
            .map(|field| RecordFieldFrame::typed(field.name.clone(), field.ty.clone()))
            .collect(),
    )
}

fn nested_spec_defs() -> Vec<String> {
    vec![
        "pub const SLOTS: usize = 2;".to_string(),
        "pub struct Inner {\n    pub value: u64,\n    pub guard: u64,\n}".to_string(),
        "pub struct Nested {\n    pub inner: Inner,\n    pub slots: [u64; SLOTS],\n    pub tag: u64,\n}"
            .to_string(),
    ]
}

fn nested_owned_frame() -> BodyObligationFrame {
    let inner = parsed_record_frame(NESTED_SOURCE, "Inner");
    let nested = parsed_record_frame(NESTED_SOURCE, "Nested");
    BodyObligationFrame {
        spec_defs: nested_spec_defs(),
        params: vec![
            BodyParamDecl::new("state", "Nested"),
            BodyParamDecl::new("index", "usize"),
            BodyParamDecl::new("next", "u64"),
        ],
        ret_type: "Nested".to_string(),
        req: Some("index < SLOTS && next < 1000".to_string()),
        named_records: vec![inner, nested.clone()],
        result_record: Some(nested),
        ..Default::default()
    }
}

fn nested_borrowed_frame() -> BodyObligationFrame {
    let inner = parsed_record_frame(NESTED_SOURCE, "Inner");
    let nested = parsed_record_frame(NESTED_SOURCE, "Nested");
    BodyObligationFrame {
        spec_defs: nested_spec_defs(),
        params: vec![
            BodyParamDecl::new("state", "&mut Nested"),
            BodyParamDecl::new("index", "usize"),
            BodyParamDecl::new("next", "u64"),
        ],
        ret_type: "u64".to_string(),
        req: Some("index < SLOTS && next < 1000".to_string()),
        fixed_array_fields: vec!["state.slots".to_string()],
        mutable_records: vec![MutableRecordFrame::new("state", nested.fields.clone())],
        mutable_call_effects: Vec::new(),
        named_records: vec![inner, nested],
        ..Default::default()
    }
}

fn verus_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("VERUS_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    Command::new("which")
        .arg("verus")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!path.is_empty()).then(|| PathBuf::from(path))
        })
}

fn assert_verus(name: &str, program: &str, should_pass: bool) {
    let Some(verus) = verus_bin() else {
        eprintln!("SKIP: verus unavailable; owned-aggregate TV `{name}` was not discharged");
        return;
    };
    let stem = format!(
        "thermite_owned_aggregate_tv_{}_{}",
        std::process::id(),
        name
    );
    let path = std::env::temp_dir().join(format!("{stem}.rs"));
    std::fs::write(&path, program).expect("write owned-aggregate TV obligation");
    let output = Command::new(verus)
        .arg(&path)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run verus for owned-aggregate TV");
    let _ = std::fs::remove_file(&path);
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        output.status.success(),
        should_pass,
        "unexpected owned-aggregate TV verdict for `{name}`:\n{combined}\n--- program ---\n{program}"
    );
}

fn state_obligation(production: &str) -> String {
    let function = function(SOURCE, 1);
    body_equivalence_obligation(
        function.body.as_ref().expect("transition body"),
        production,
        &state_frame(),
    )
    .expect("owned-record body obligation must build")
}

#[test]
fn owned_record_update_and_dependent_read_are_l3_faithful() {
    let program = state_obligation(
        "    let mut updated: State = state;\n\
         updated.first = next;\n\
         updated.second = updated.first;\n\
         updated",
    );
    assert!(program.contains("result.first =="), "{program}");
    assert!(program.contains("result.second =="), "{program}");
    assert!(program.contains("result.occupied =="), "{program}");
    assert_verus("faithful", &program, true);
}

#[test]
fn owned_record_mutants_fail_the_all_input_l3_obligation() {
    for (name, production) in [
        (
            "dropped",
            "    let mut updated: State = state;\n    updated.first = next;\n    updated",
        ),
        (
            "wrong_field",
            "    let mut updated: State = state;\n    updated.second = next;\n    updated",
        ),
        (
            "wrong_value",
            "    let mut updated: State = state;\n    updated.first = 0;\n    updated.second = updated.first;\n    updated",
        ),
        (
            "reordered",
            "    let mut updated: State = state;\n    updated.second = updated.first;\n    updated.first = next;\n    updated",
        ),
        (
            "collateral",
            "    let mut updated: State = state;\n    updated.first = next;\n    updated.second = updated.first;\n    updated.occupied = false;\n    updated",
        ),
        (
            "stale_read",
            "    let stale: u64 = state.first;\n    let mut updated: State = state;\n    updated.first = next;\n    updated.second = stale;\n    updated",
        ),
    ] {
        assert_verus(name, &state_obligation(production), false);
    }
}

#[test]
fn fixed_array_field_replacement_has_an_exact_l3_frame() {
    let function = function(ARRAY_SOURCE, 2);
    let faithful = body_equivalence_obligation(
        function.body.as_ref().expect("replace_slot body"),
        "    let mut slots: [u64; SLOTS] = state.slots;\n\
         slots[index] = value;\n\
         let mut updated: Store = state;\n\
         updated.slots = slots;\n\
         updated",
        &store_frame(),
    )
    .expect("array-field body obligation must build");
    assert!(faithful.contains("result.slots@ =="), "{faithful}");
    assert!(faithful.contains("result.tag =="), "{faithful}");
    assert_verus("array_field_faithful", &faithful, true);

    for (name, production) in [
        (
            "array_field_dropped",
            "    let mut slots: [u64; SLOTS] = state.slots;\n    slots[index] = value;\n    let mut updated: Store = state;\n    updated",
        ),
        (
            "array_field_wrong_value",
            "    let mut slots: [u64; SLOTS] = state.slots;\n    slots[index] = 0;\n    let mut updated: Store = state;\n    updated.slots = slots;\n    updated",
        ),
    ] {
        let program = body_equivalence_obligation(
            function.body.as_ref().expect("replace_slot body"),
            production,
            &store_frame(),
        )
        .expect("array-field mutant obligation must build");
        assert_verus(name, &program, false);
    }
}

fn nested_owned_obligation(production: &str) -> String {
    let function = function(NESTED_SOURCE, 3);
    body_equivalence_obligation(
        function.body.as_ref().expect("nested_owned body"),
        production,
        &nested_owned_frame(),
    )
    .expect("nested owned-record body obligation must build")
}

#[test]
fn nested_record_and_terminal_array_writes_are_l3_faithful() {
    let program = nested_owned_obligation(
        "    let mut updated: Nested = state;\n\
         updated.inner.value = next;\n\
         updated.slots[index] = updated.inner.value + 1;\n\
         updated",
    );
    assert!(program.contains("spec_array_update"), "{program}");
    assert!(program.contains("result.inner =="), "{program}");
    assert!(program.contains("result.slots@ =="), "{program}");
    assert_verus("nested_owned_faithful", &program, true);
}

#[test]
fn nested_owned_mutants_fail_the_all_input_l3_obligation() {
    for (name, production) in [
        (
            "nested_dropped_inner",
            "    let mut updated: Nested = state;\n    updated.slots[index] = updated.inner.value + 1;\n    updated",
        ),
        (
            "nested_dropped_array",
            "    let mut updated: Nested = state;\n    updated.inner.value = next;\n    updated",
        ),
        (
            "nested_wrong_index",
            "    let mut updated: Nested = state;\n    updated.inner.value = next;\n    updated.slots[0] = updated.inner.value + 1;\n    updated",
        ),
        (
            "nested_wrong_value",
            "    let mut updated: Nested = state;\n    updated.inner.value = next;\n    updated.slots[index] = updated.inner.value;\n    updated",
        ),
        (
            "nested_reordered_dependent_read",
            "    let mut updated: Nested = state;\n    updated.slots[index] = updated.inner.value;\n    updated.inner.value = next;\n    updated",
        ),
        (
            "nested_collateral_guard",
            "    let mut updated: Nested = state;\n    updated.inner.value = next;\n    updated.inner.guard = next;\n    updated.slots[index] = updated.inner.value + 1;\n    updated",
        ),
        (
            "nested_collateral_tag",
            "    let mut updated: Nested = state;\n    updated.inner.value = next;\n    updated.slots[index] = updated.inner.value + 1;\n    updated.tag = next;\n    updated",
        ),
    ] {
        assert_verus(name, &nested_owned_obligation(production), false);
    }
}

fn nested_borrowed_obligation(production: &str) -> String {
    let function = function(NESTED_SOURCE, 4);
    body_equivalence_obligation(
        function.body.as_ref().expect("nested_borrowed body"),
        production,
        &nested_borrowed_frame(),
    )
    .expect("nested borrowed-record body obligation must build")
}

#[test]
fn exclusive_nested_record_and_array_writes_are_l3_faithful() {
    let faithful = nested_borrowed_obligation(
        "    state.inner.value = next;\n\
         state.slots[index] = state.inner.value + 1;\n\
         state.inner.value",
    );
    assert!(
        faithful.contains("final(state).inner.value =="),
        "{faithful}"
    );
    assert!(
        faithful.contains("final(state).inner.guard =="),
        "{faithful}"
    );
    assert!(faithful.contains("final(state).slots@ =="), "{faithful}");
    assert_verus("nested_borrowed_faithful", &faithful, true);

    for (name, production) in [
        (
            "nested_borrowed_dropped_array",
            "    state.inner.value = next;\n    state.inner.value",
        ),
        (
            "nested_borrowed_collateral_guard",
            "    state.inner.value = next;\n    state.inner.guard = next;\n    state.slots[index] = state.inner.value + 1;\n    state.inner.value",
        ),
    ] {
        assert_verus(name, &nested_borrowed_obligation(production), false);
    }
}

#[test]
fn aggregate_exec_call_comparison_rejects_the_wrong_callee() {
    let parsed = thermite_syntax::parse(
        "struct State { first: u64, second: u64, occupied: bool }\n\
         fn expected(state: State) -> State req true ens result.first == state.first \
         ens result.second == state.second ens result.occupied == state.occupied fx pure \
         { state }\n",
    );
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected function");
    };
    let call = function.body.as_ref().unwrap().tail.as_ref().unwrap();
    let record = NamedRecordFrame::new(
        "State",
        vec![
            RecordFieldFrame::new("first", false),
            RecordFieldFrame::new("second", false),
            RecordFieldFrame::new("occupied", false),
        ],
    );
    let frame = ExecObligationFrame {
        spec_defs: vec![
            "pub struct State {\n    pub first: u64,\n    pub second: u64,\n    pub occupied: bool,\n}"
                .to_string(),
            r#"
fn expected(state: State) -> (result: State)
    ensures
        result.first == state.first,
        result.second == state.second,
        result.occupied == state.occupied,
{
    state
}
fn wrong(state: State) -> (result: State)
    ensures
        result.first == state.second,
        result.second == state.second,
        result.occupied == state.occupied,
{
    State { first: state.second, second: state.second, occupied: state.occupied }
}
"#
            .to_string(),
        ],
        params: vec![ExecParamDecl::new("state", "State")],
        ret_type: "State".to_string(),
        result_record: Some(record),
        ..Default::default()
    };
    let faithful = exec_equivalence_obligation(call, "expected(state)", &frame)
        .expect("aggregate-call obligation must build");
    assert_verus("aggregate_call_faithful", &faithful, true);
    let wrong = exec_equivalence_obligation(call, "wrong(state)", &frame)
        .expect("wrong-callee obligation must build");
    assert_verus("aggregate_call_wrong_callee", &wrong, false);
}
