use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::ast::{Block, Expr, PrimType, Type};
use thermite_syntax::Item;
use thermite_tv::obligation::{
    body_equivalence_obligation, exec_equivalence_obligation, BodyObligationFrame, BodyParamDecl,
    ExecObligationFrame, ExecParamDecl,
};
use thermite_tv::{
    MutableCallEffectFrame, MutableIndexedFrame, MutableRecordFrame, NamedRecordFrame,
    RecordFieldFrame, SharedIndexedFrame, SharedRecordFrame,
};

const SOURCE: &str = r#"
struct State { value: u64, guard: u64 }
fn mutate(state: &mut State, value: u64) -> ()
  req true
  ens final(state).value == value
  ens final(state).guard == old(state).guard
  fx pure
{
  state.value = value;
}
fn mutate_returning(state: &mut State, value: u64) -> u64
  req true
  ens result == value
  ens final(state).value == value
  ens final(state).guard == old(state).guard
  fx pure
{
  state.value = value;
  value
}
fn pipeline(state: &mut State, value: u64) -> ()
  req value < 1000
  ens final(state).value == value + 1
  ens final(state).guard == old(state).guard
  fx pure
{
  mutate(state, value);
  let next: u64 = state.value + 1;
  mutate(state, next);
}
fn result_pipeline(state: &mut State, value: u64) -> u64
  req value < 1000
  ens result == value + 1
  ens final(state).value == result
  ens final(state).guard == old(state).guard
  fx pure
{
  let observed: u64 = mutate_returning(state, value);
  let next: u64 = observed + 1;
  let final_value: u64 = mutate_returning(state, next);
  final_value
}
fn copy_from(left: &mut State, right: &State) -> u64
  req true
  ens result == right.guard
  ens final(left).value == right.value
  ens final(left).guard == old(left).guard
  fx pure
{
  left.value = right.value;
  right.guard
}
fn mixed_pipeline(left: &mut State, right: &State) -> u64
  req true
  ens result == right.guard
  ens final(left).value == right.value
  ens final(left).guard == old(left).guard
  fx pure
{
  let observed: u64 = copy_from(left, right);
  observed
}
"#;

const DEFINITIONS: &str = r#"
pub struct State {
    pub value: u64,
    pub guard: u64,
}

fn mutate(state: &mut State, value: u64) -> (result: ())
    ensures
        final(state).value == value,
        final(state).guard == old(state).guard,
{
    state.value = value;
}

fn mutate_returning(state: &mut State, value: u64) -> (result: u64)
    ensures
        result == value,
        final(state).value == value,
        final(state).guard == old(state).guard,
{
    state.value = value;
    value
}

fn copy_from(left: &mut State, right: &State) -> (result: u64)
    ensures
        result == right.guard,
        final(left).value == right.value,
        final(left).guard == old(left).guard,
{
    left.value = right.value;
    right.guard
}
"#;

const PROJECTED_SOURCE: &str = r#"
struct Leaf { value: u64, guard: u64 }
struct Pair { left: Leaf, right: Leaf }
struct Outer { pair: Pair, tag: u64 }
fn set_leaf(leaf: &mut Leaf, value: u64) -> u64
  req true
  ens result == value
  ens final(leaf).value == value
  ens final(leaf).guard == old(leaf).guard
  fx pure
{
  leaf.value = value;
  leaf.value
}
fn copy_leaf(destination: &mut Leaf, source: &Leaf) -> u64
  req true
  ens result == source.value
  ens final(destination).value == source.value
  ens final(destination).guard == old(destination).guard
  fx pure
{
  destination.value = source.value;
  destination.value
}
fn projected_pipeline(outer: &mut Outer, value: u64) -> u64
  req value < 1000
  ens result == value
  ens final(outer).pair.left.value == value
  ens final(outer).pair.right.value == value
  ens final(outer).tag == old(outer).tag
  fx pure
{
  let written: u64 = set_leaf(&mut outer.pair.left, value);
  let observed: u64 = copy_leaf(&mut outer.pair.right, &outer.pair.left);
  observed
}
"#;

const PROJECTED_DEFINITIONS: &str = r#"
pub struct Leaf { pub value: u64, pub guard: u64 }
pub struct Pair { pub left: Leaf, pub right: Leaf }
pub struct Outer { pub pair: Pair, pub tag: u64 }

fn set_leaf(leaf: &mut Leaf, value: u64) -> (result: u64)
    ensures
        result == value,
        final(leaf).value == value,
        final(leaf).guard == old(leaf).guard,
{
    leaf.value = value;
    leaf.value
}

fn copy_leaf(destination: &mut Leaf, source: &Leaf) -> (result: u64)
    ensures
        result == source.value,
        final(destination).value == source.value,
        final(destination).guard == old(destination).guard,
{
    destination.value = source.value;
    destination.value
}
"#;

const PROJECTED_INDEXED_SOURCE: &str = r#"
const SLOTS: usize = 2;
struct Bank { slots: [u64; SLOTS], guard: u64 }
struct ArrayOuter { left: Bank, right: Bank, tag: u64 }
fn write_array(data: &mut [u64; SLOTS], value: u64) -> u64
  req true
  ens result == value
  ens final(data)[0] == value
  ens final(data)[1] == old(data)[1]
  fx pure
{
  data[0] = value;
  data[0]
}
fn copy_array(destination: &mut [u64; SLOTS], source: &[u64; SLOTS]) -> u64
  req true
  ens result == source[0]
  ens final(destination)[0] == source[0]
  ens final(destination)[1] == old(destination)[1]
  fx pure
{
  destination[0] = source[0];
  destination[0]
}
fn projected_array_pipeline(outer: &mut ArrayOuter, value: u64) -> u64
  req value < 1000
  ens result == value
  ens final(outer).left.slots[0] == value
  ens final(outer).right.slots[0] == value
  ens final(outer).left.guard == old(outer).left.guard
  ens final(outer).right.guard == old(outer).right.guard
  ens final(outer).tag == old(outer).tag
  fx pure
{
  let written: u64 = write_array(&mut outer.left.slots, value);
  let observed: u64 = copy_array(&mut outer.right.slots, &outer.left.slots);
  observed
}
"#;

const PROJECTED_INDEXED_DEFINITIONS: &str = r#"
pub const SLOTS: usize = 2;
pub struct Bank { pub slots: [u64; SLOTS], pub guard: u64 }
pub struct ArrayOuter { pub left: Bank, pub right: Bank, pub tag: u64 }

fn write_array(data: &mut [u64; SLOTS], value: u64) -> (result: u64)
    ensures
        result == value,
        final(data)@[0] == value,
        final(data)@[1] == old(data)@[1],
{
    data[0] = value;
    data[0]
}

fn copy_array(destination: &mut [u64; SLOTS], source: &[u64; SLOTS]) -> (result: u64)
    ensures
        result == source@[0],
        final(destination)@[0] == source@[0],
        final(destination)@[1] == old(destination)@[1],
{
    destination[0] = source[0];
    destination[0]
}
"#;

const PROJECTED_INDEXED_BRANCH_SOURCE: &str = r#"
fn replace_one_branch(outer: &mut ArrayOuter, value: u64, choose: bool) -> u64
  req value < 1000
  ens result == value
  fx pure
{
  let written: u64 = write_array(&mut outer.left.slots, value);
  if choose {
    outer.left.slots = [value, 1];
  }
  written
}

fn replace_both_branches(outer: &mut ArrayOuter, value: u64, choose: bool) -> u64
  req value < 1000
  ens result == value
  fx pure
{
  let written: u64 = write_array(&mut outer.left.slots, value);
  if choose {
    outer.left.slots = [value, 1];
  } else {
    outer.left.slots = [value, 2];
  }
  written
}
"#;

fn function_body(name: &str) -> Block {
    function_body_in(SOURCE, name)
}

fn function_body_in(source: &str, name: &str) -> Block {
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.name == name => function.body.clone(),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn leaf_fields() -> Vec<RecordFieldFrame> {
    vec![
        RecordFieldFrame::typed("value", Type::Prim(PrimType::U64)),
        RecordFieldFrame::typed("guard", Type::Prim(PrimType::U64)),
    ]
}

fn pair_fields() -> Vec<RecordFieldFrame> {
    vec![
        RecordFieldFrame::typed("left", Type::Named("Leaf".to_string())),
        RecordFieldFrame::typed("right", Type::Named("Leaf".to_string())),
    ]
}

fn outer_fields() -> Vec<RecordFieldFrame> {
    vec![
        RecordFieldFrame::typed("pair", Type::Named("Pair".to_string())),
        RecordFieldFrame::typed("tag", Type::Prim(PrimType::U64)),
    ]
}

fn projected_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![PROJECTED_DEFINITIONS.to_string()],
        params: vec![
            BodyParamDecl::new("outer", "&mut Outer"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "u64".to_string(),
        req: Some("value < 1000".to_string()),
        mutable_records: vec![MutableRecordFrame::typed("outer", "Outer", outer_fields())],
        mutable_call_effects: vec![
            MutableCallEffectFrame::new(
                "set_leaf",
                vec!["leaf".to_string(), "value".to_string()],
                vec![MutableRecordFrame::typed("leaf", "Leaf", leaf_fields())],
                function_body_in(PROJECTED_SOURCE, "set_leaf"),
            ),
            MutableCallEffectFrame::new(
                "copy_leaf",
                vec!["destination".to_string(), "source".to_string()],
                vec![MutableRecordFrame::typed(
                    "destination",
                    "Leaf",
                    leaf_fields(),
                )],
                function_body_in(PROJECTED_SOURCE, "copy_leaf"),
            )
            .with_shared_records(vec![SharedRecordFrame::typed(
                "source",
                "Leaf",
                leaf_fields(),
            )]),
        ],
        named_records: vec![
            NamedRecordFrame::new("Leaf", leaf_fields()),
            NamedRecordFrame::new("Pair", pair_fields()),
            NamedRecordFrame::new("Outer", outer_fields()),
        ],
        ..Default::default()
    }
}

fn projected_indexed_frame() -> BodyObligationFrame {
    let parsed = thermite_syntax::parse(PROJECTED_INDEXED_SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let structure_fields = |name: &str| {
        parsed
            .program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Struct(structure) if structure.name == name => Some(
                    structure
                        .fields
                        .iter()
                        .map(|field| RecordFieldFrame::typed(field.name.clone(), field.ty.clone()))
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing projected indexed structure `{name}`"))
    };
    let bank_fields = structure_fields("Bank");
    let outer_fields = structure_fields("ArrayOuter");
    let slots_type = bank_fields
        .iter()
        .find(|field| field.name == "slots")
        .and_then(|field| field.ty.clone())
        .expect("Bank.slots exact type");

    BodyObligationFrame {
        spec_defs: vec![PROJECTED_INDEXED_DEFINITIONS.to_string()],
        params: vec![
            BodyParamDecl::new("outer", "&mut ArrayOuter"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "u64".to_string(),
        req: Some("value < 1000".to_string()),
        fixed_array_fields: vec![
            "outer.left.slots".to_string(),
            "outer.right.slots".to_string(),
        ],
        mutable_records: vec![MutableRecordFrame::typed(
            "outer",
            "ArrayOuter",
            outer_fields.clone(),
        )],
        mutable_call_effects: vec![
            MutableCallEffectFrame::new(
                "write_array",
                vec!["data".to_string(), "value".to_string()],
                vec![],
                function_body_in(PROJECTED_INDEXED_SOURCE, "write_array"),
            )
            .with_mutable_indexed(vec![MutableIndexedFrame::new("data", slots_type.clone())]),
            MutableCallEffectFrame::new(
                "copy_array",
                vec!["destination".to_string(), "source".to_string()],
                vec![],
                function_body_in(PROJECTED_INDEXED_SOURCE, "copy_array"),
            )
            .with_mutable_indexed(vec![MutableIndexedFrame::new(
                "destination",
                slots_type.clone(),
            )])
            .with_shared_indexed(vec![SharedIndexedFrame::new("source", slots_type)]),
        ],
        named_records: vec![
            NamedRecordFrame::new("Bank", bank_fields),
            NamedRecordFrame::new("ArrayOuter", outer_fields),
        ],
        ..Default::default()
    }
}

fn fields() -> Vec<RecordFieldFrame> {
    vec![
        RecordFieldFrame::typed("value", Type::Prim(PrimType::U64)),
        RecordFieldFrame::typed("guard", Type::Prim(PrimType::U64)),
    ]
}

fn effect(name: &str, formals: &[&str], mutable: &[&str], body: Block) -> MutableCallEffectFrame {
    MutableCallEffectFrame::new(
        name,
        formals.iter().map(|formal| (*formal).to_string()).collect(),
        mutable
            .iter()
            .map(|formal| MutableRecordFrame::typed(*formal, "State", fields()))
            .collect(),
        body,
    )
}

fn frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![DEFINITIONS.to_string()],
        params: vec![
            BodyParamDecl::new("state", "&mut State"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "()".to_string(),
        req: Some("value < 1000".to_string()),
        result_is_unit: true,
        mutable_records: vec![MutableRecordFrame::typed("state", "State", fields())],
        mutable_call_effects: vec![effect(
            "mutate",
            &["state", "value"],
            &["state"],
            function_body("mutate"),
        )],
        named_records: vec![NamedRecordFrame::new("State", fields())],
        ..Default::default()
    }
}

fn result_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![DEFINITIONS.to_string()],
        params: vec![
            BodyParamDecl::new("state", "&mut State"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "u64".to_string(),
        req: Some("value < 1000".to_string()),
        mutable_records: vec![MutableRecordFrame::typed("state", "State", fields())],
        mutable_call_effects: vec![effect(
            "mutate_returning",
            &["state", "value"],
            &["state"],
            function_body("mutate_returning"),
        )],
        named_records: vec![NamedRecordFrame::new("State", fields())],
        ..Default::default()
    }
}

fn mixed_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![DEFINITIONS.to_string()],
        params: vec![
            BodyParamDecl::new("left", "&mut State"),
            BodyParamDecl::new("right", "&State"),
        ],
        ret_type: "u64".to_string(),
        mutable_records: vec![MutableRecordFrame::typed("left", "State", fields())],
        shared_records: vec![SharedRecordFrame::typed("right", "State", fields())],
        mutable_call_effects: vec![MutableCallEffectFrame::new(
            "copy_from",
            vec!["left".to_string(), "right".to_string()],
            vec![MutableRecordFrame::typed("left", "State", fields())],
            function_body("copy_from"),
        )
        .with_shared_records(vec![SharedRecordFrame::typed("right", "State", fields())])],
        named_records: vec![NamedRecordFrame::new("State", fields())],
        ..Default::default()
    }
}

fn faithful_production() -> &'static str {
    r#"    mutate(state, value);
    let next: u64 = state.value + 1;
    mutate(state, next);
"#
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
        eprintln!("SKIP: verus unavailable; mutable-call TV `{name}` was not discharged");
        return;
    };
    let path = std::env::temp_dir().join(format!(
        "thermite_mutable_call_tv_{}_{}.rs",
        std::process::id(),
        name
    ));
    std::fs::write(&path, program).expect("write mutable-call TV obligation");
    let output = Command::new(verus)
        .arg(&path)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run verus for mutable-call TV");
    let _ = std::fs::remove_file(&path);
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        output.status.success(),
        should_pass,
        "unexpected mutable-call TV verdict for `{name}`:\n{combined}\n--- program ---\n{program}"
    );
}

#[test]
fn exact_mutable_call_effect_and_dependent_sequence_verify() {
    let obligation =
        body_equivalence_obligation(&function_body("pipeline"), faithful_production(), &frame())
            .expect("exact mutable-call obligation");
    assert!(
        obligation.contains("final(state).value == (value + 1)"),
        "{obligation}"
    );
    assert!(
        obligation.contains("final(state).guard == old(state).guard"),
        "{obligation}"
    );
    assert_verus("faithful", &obligation, true);
}

#[test]
fn direct_let_bound_mutable_call_results_compose_with_exact_post_state() {
    let source = function_body("result_pipeline");
    let production = r#"    let observed: u64 = mutate_returning(state, value);
    let next: u64 = observed + 1;
    let final_value: u64 = mutate_returning(state, next);
    final_value
"#;
    let obligation = body_equivalence_obligation(&source, production, &result_frame())
        .expect("result-consuming mutable-call obligation");
    assert!(obligation.contains("result == (value + 1)"), "{obligation}");
    assert!(
        obligation.contains("final(state).value == (value + 1)"),
        "{obligation}"
    );
    assert!(
        obligation.contains("final(state).guard == old(state).guard"),
        "{obligation}"
    );
    assert_verus("result_and_post_state", &obligation, true);

    for (name, mutant) in [
        (
            "wrong_consumed_result",
            r#"    let observed: u64 = mutate_returning(state, value);
    let next: u64 = observed + 2;
    let final_value: u64 = mutate_returning(state, next);
    final_value
"#,
        ),
        (
            "discarded_return_value",
            r#"    let observed: u64 = mutate_returning(state, value);
    let next: u64 = observed + 1;
    let final_value: u64 = mutate_returning(state, next);
    value
"#,
        ),
    ] {
        let obligation = body_equivalence_obligation(&source, mutant, &result_frame())
            .expect("result-consuming mutant obligation");
        assert_verus(name, &obligation, false);
    }
}

#[test]
fn mixed_shared_and_mutable_records_compose_snapshot_result_and_post_state() {
    let production = r#"    let observed: u64 = copy_from(left, right);
    observed
"#;
    let obligation =
        body_equivalence_obligation(&function_body("mixed_pipeline"), production, &mixed_frame())
            .expect("mixed shared/mutable call obligation");
    assert!(obligation.contains("result == right.guard"), "{obligation}");
    assert!(
        obligation.contains("final(left).value == right.value"),
        "{obligation}"
    );
    assert!(
        obligation.contains("final(left).guard == old(left).guard"),
        "{obligation}"
    );
    assert_verus("mixed_shared_mutable", &obligation, true);

    let wrong_result = body_equivalence_obligation(
        &function_body("mixed_pipeline"),
        "    let observed: u64 = copy_from(left, right);\n    left.guard\n",
        &mixed_frame(),
    )
    .expect("mixed result mutant obligation");
    assert_verus("mixed_wrong_shared_result", &wrong_result, false);
}

#[test]
fn projected_record_calls_compose_current_sibling_state_at_l3() {
    let body = function_body_in(PROJECTED_SOURCE, "projected_pipeline");
    let obligation = body_equivalence_obligation(
        &body,
        r#"    let written: u64 = set_leaf(&mut outer.pair.left, value);
    let observed: u64 = copy_leaf(&mut outer.pair.right, &outer.pair.left);
    observed
"#,
        &projected_frame(),
    )
    .expect("nested projected-record call obligation");
    assert_verus("projected_record_calls", &obligation, true);

    let mutant = body_equivalence_obligation(
        &body,
        r#"    let written: u64 = set_leaf(&mut outer.pair.left, value + 1);
    let observed: u64 = copy_leaf(&mut outer.pair.right, &outer.pair.left);
    observed
"#,
        &projected_frame(),
    )
    .expect("projected-record mutant obligation");
    assert_verus("projected_record_calls_mutant", &mutant, false);
}

#[test]
fn projected_indexed_calls_compose_nested_sequence_state_at_l3() {
    let body = function_body_in(PROJECTED_INDEXED_SOURCE, "projected_array_pipeline");
    let production = r#"    let written: u64 = write_array(&mut outer.left.slots, value);
    let observed: u64 = copy_array(&mut outer.right.slots, &outer.left.slots);
    observed
"#;
    let obligation = body_equivalence_obligation(&body, production, &projected_indexed_frame())
        .expect("nested projected-indexed call obligation");
    assert!(
        obligation.contains("final(outer).left.slots@ =="),
        "{obligation}"
    );
    assert!(
        obligation.contains("final(outer).right.slots@ =="),
        "{obligation}"
    );
    assert!(
        obligation.contains("final(outer).left.guard == (old(outer).left).guard"),
        "{obligation}"
    );
    assert_verus("projected_indexed_calls", &obligation, true);

    let mutant = body_equivalence_obligation(
        &body,
        r#"    let written: u64 = write_array(&mut outer.left.slots, value + 1);
    let observed: u64 = copy_array(&mut outer.right.slots, &outer.left.slots);
    observed
"#,
        &projected_indexed_frame(),
    )
    .expect("projected-indexed mutant obligation");
    assert_verus("projected_indexed_calls_mutant", &mutant, false);
}

#[test]
fn projected_indexed_branch_overlay_lifecycle_is_exact_or_fails_closed() {
    let mut frame = projected_indexed_frame();
    frame.params.push(BodyParamDecl::new("choose", "bool"));

    let one_branch = function_body_in(PROJECTED_INDEXED_BRANCH_SOURCE, "replace_one_branch");
    let error = body_equivalence_obligation(
        &one_branch,
        r#"    let written: u64 = write_array(&mut outer.left.slots, value);
    if choose {
        outer.left.slots = [value, 1];
    }
    written
"#,
        &frame,
    )
    .expect_err("one-branch overlay removal must fail closed");
    assert!(
        error
            .to_string()
            .contains("changes projected indexed path `outer.left.slots` in only one branch"),
        "{error}"
    );

    let both_branches = function_body_in(PROJECTED_INDEXED_BRANCH_SOURCE, "replace_both_branches");
    let obligation = body_equivalence_obligation(
        &both_branches,
        r#"    let written: u64 = write_array(&mut outer.left.slots, value);
    if choose {
        outer.left.slots = [value, 1];
    } else {
        outer.left.slots = [value, 2];
    }
    written
"#,
        &frame,
    )
    .expect("symmetric overlay removal uses exact merged native arrays");
    assert_verus(
        "projected_indexed_branch_overlay_lifecycle",
        &obligation,
        true,
    );
}

#[test]
fn projected_record_type_and_borrow_mode_mismatches_fail_closed() {
    for (name, call, expected) in [
        (
            "wrong_projected_type",
            "set_leaf(&mut outer.pair, value);",
            "expects `Leaf`",
        ),
        (
            "wrong_projected_borrow",
            "set_leaf(&outer.pair.left, value);",
            "exclusive record formal `set_leaf::leaf` received a shared projected borrow",
        ),
    ] {
        let parsed = thermite_syntax::parse(&format!(
            r#"
fn {name}(outer: &mut Outer, value: u64) -> ()
  req true
  ens true
  fx pure
{{
  {call}
}}
"#
        ));
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let body = parsed
            .program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.name == name => function.body.clone(),
                _ => None,
            })
            .expect("projected mismatch body");
        let mut frame = projected_frame();
        frame.ret_type = "()".to_string();
        frame.result_is_unit = true;
        frame.req = None;
        let error = body_equivalence_obligation(&body, "", &frame)
            .expect_err("invalid projected record actual must fail closed");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn shared_actual_may_not_overlap_an_exclusive_actual() {
    let parsed = thermite_syntax::parse(
        r#"
fn alias(state: &mut State) -> u64
  req true
  ens result == final(state).guard
  fx pure
{
  let observed: u64 = copy_from(state, state);
  observed
}
"#,
    );
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let body = parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.name == "alias" => function.body.clone(),
            _ => None,
        })
        .expect("alias body");
    let mut frame = mixed_frame();
    frame.params = vec![BodyParamDecl::new("state", "&mut State")];
    frame.mutable_records = vec![MutableRecordFrame::typed("state", "State", fields())];
    frame.shared_records.clear();
    let error = body_equivalence_obligation(&body, "", &frame)
        .expect_err("shared/exclusive overlap must fail before certification");
    assert!(
        error
            .to_string()
            .contains("aliases exclusive access path `state` through shared actual `state`"),
        "{error}"
    );
}

#[test]
fn nested_mutable_call_result_use_remains_fail_closed() {
    let source = thermite_syntax::parse(
        r#"
fn nested(state: &mut State, value: u64) -> u64
  req true
  ens result == value + 1
  fx pure
{
  let observed: u64 = mutate_returning(state, value) + 1;
  observed
}
"#,
    );
    assert!(source.is_clean(), "parse errors: {:?}", source.errors);
    let body = source
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.name == "nested" => function.body.clone(),
            _ => None,
        })
        .expect("nested body");
    let error = body_equivalence_obligation(&body, "", &result_frame())
        .expect_err("nested mutable-call result use must not silently lose its effect");
    assert!(
        error
            .to_string()
            .contains("may only be consumed as one direct `let` initializer"),
        "{error}"
    );

    let untyped = thermite_syntax::parse(
        r#"
fn untyped(state: &mut State, value: u64) -> u64
  req true
  ens result == value
  fx pure
{
  let observed = mutate_returning(state, value);
  observed
}
"#,
    );
    assert!(untyped.is_clean(), "parse errors: {:?}", untyped.errors);
    let body = untyped
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.name == "untyped" => function.body.clone(),
            _ => None,
        })
        .expect("untyped body");
    let error = body_equivalence_obligation(&body, "", &result_frame())
        .expect_err("untyped mutable-call result binding must remain fail closed");
    assert!(
        error
            .to_string()
            .contains("requires one direct typed `let` initializer"),
        "{error}"
    );
}

#[test]
fn exec_reference_selects_the_final_mutable_record_field() {
    let expression = Expr::Field {
        receiver: Box::new(Expr::Path(vec!["state".to_string()])),
        name: "value".to_string(),
    };
    let exact_frame = ExecObligationFrame {
        spec_defs: vec!["pub struct State { pub value: u64, pub guard: u64 }".to_string()],
        params: vec![ExecParamDecl::new("state", "&mut State")],
        ret_type: "u64".to_string(),
        mutable_records: vec![MutableRecordFrame::typed("state", "State", fields())],
        ..Default::default()
    };
    let obligation = exec_equivalence_obligation(&expression, "state.value", &exact_frame)
        .expect("mutable field exec obligation");
    assert!(
        obligation.contains("ensures result == final(state).value"),
        "{obligation}"
    );
    assert_verus("exec_final_field", &obligation, true);

    let mut unframed = exact_frame;
    unframed.mutable_records.clear();
    let unframed_obligation = exec_equivalence_obligation(&expression, "state.value", &unframed)
        .expect("unframed mutable field obligation");
    assert!(!unframed_obligation.contains("final(state).value"));
    assert_verus("exec_missing_final_field", &unframed_obligation, false);
}

#[test]
fn dropped_wrong_argument_and_missing_frame_mutants_fail() {
    let source = function_body("pipeline");
    for (name, production, definitions) in [
        (
            "dropped_second_call",
            "    mutate(state, value);\n",
            DEFINITIONS,
        ),
        (
            "wrong_second_argument",
            "    mutate(state, value);\n    let next: u64 = state.value + 1;\n    mutate(state, value);\n",
            DEFINITIONS,
        ),
        (
            "missing_collateral_frame",
            faithful_production(),
            r#"
pub struct State { pub value: u64, pub guard: u64 }
fn mutate(state: &mut State, value: u64) -> (result: ())
    ensures final(state).value == value,
{
    state.value = value;
}
"#,
        ),
    ] {
        let mut mutant_frame = frame();
        mutant_frame.spec_defs = vec![definitions.to_string()];
        let obligation = body_equivalence_obligation(&source, production, &mutant_frame)
            .expect("mutable-call mutant obligation");
        assert_verus(name, &obligation, false);
    }
}

#[test]
fn exclusive_alias_and_recursive_effect_cycles_fail_closed() {
    let alias_source = r#"
fn touch_two(left: &mut State, right: &mut State, value: u64) -> ()
  req true
  ens final(left).value == value
  ens final(left).guard == old(left).guard
  ens final(right).guard == value
  ens final(right).value == old(right).value
  fx pure
{
  left.value = value;
  right.guard = value;
}
fn distinct(left: &mut State, right: &mut State, value: u64) -> ()
  req true
  ens final(left).value == value
  ens final(left).guard == old(left).guard
  ens final(right).guard == value
  ens final(right).value == old(right).value
  fx pure
{
  touch_two(left, right, value);
}
fn alias(state: &mut State, value: u64) -> ()
  req true
  ens final(state).value == value
  fx pure
{
  touch_two(state, state, value);
}
"#;
    let parsed = thermite_syntax::parse(alias_source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let touch = parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.name == "touch_two" => function.body.clone(),
            _ => None,
        })
        .expect("touch_two body");
    let alias = parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.name == "alias" => function.body.clone(),
            _ => None,
        })
        .expect("alias body");
    let distinct = parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.name == "distinct" => function.body.clone(),
            _ => None,
        })
        .expect("distinct body");
    let mut distinct_frame = BodyObligationFrame {
        spec_defs: vec![r#"
pub struct State { pub value: u64, pub guard: u64 }
fn touch_two(left: &mut State, right: &mut State, value: u64) -> (result: ())
    ensures
        final(left).value == value,
        final(left).guard == old(left).guard,
        final(right).guard == value,
        final(right).value == old(right).value,
{
    left.value = value;
    right.guard = value;
}
"#
        .to_string()],
        params: vec![
            BodyParamDecl::new("left", "&mut State"),
            BodyParamDecl::new("right", "&mut State"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "()".to_string(),
        result_is_unit: true,
        mutable_records: vec![
            MutableRecordFrame::typed("left", "State", fields()),
            MutableRecordFrame::typed("right", "State", fields()),
        ],
        named_records: vec![NamedRecordFrame::new("State", fields())],
        ..Default::default()
    };
    distinct_frame.mutable_call_effects = vec![effect(
        "touch_two",
        &["left", "right", "value"],
        &["left", "right"],
        touch.clone(),
    )];
    let obligation = body_equivalence_obligation(
        &distinct,
        "    touch_two(left, right, value);\n",
        &distinct_frame,
    )
    .expect("pairwise-distinct mutable roots compose");
    assert_verus("distinct_exclusive_roots", &obligation, true);

    let mut alias_frame = frame();
    alias_frame.req = None;
    alias_frame.mutable_call_effects = vec![effect(
        "touch_two",
        &["left", "right", "value"],
        &["left", "right"],
        touch,
    )];
    let error = body_equivalence_obligation(&alias, "", &alias_frame)
        .expect_err("two mutable formals may not alias one root");
    assert!(
        error.to_string().contains("aliases exclusive access paths"),
        "{error}"
    );

    let mut recursive_frame = frame();
    recursive_frame.mutable_call_effects = vec![effect(
        "mutate",
        &["state", "value"],
        &["state"],
        function_body("pipeline"),
    )];
    let error = body_equivalence_obligation(
        &function_body("pipeline"),
        faithful_production(),
        &recursive_frame,
    )
    .expect_err("recursive mutable effect cycle must fail closed");
    assert!(error.to_string().contains("effect cycle"), "{error}");
}
