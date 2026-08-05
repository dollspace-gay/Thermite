use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::ast::{Block, Expr, PrimType, Type};
use thermite_syntax::Item;
use thermite_tv::obligation::{
    body_equivalence_obligation, exec_equivalence_obligation, BodyObligationFrame, BodyParamDecl,
    ExecObligationFrame, ExecParamDecl,
};
use thermite_tv::{MutableCallEffectFrame, MutableRecordFrame, NamedRecordFrame, RecordFieldFrame};

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
"#;

fn function_body(name: &str) -> Block {
    let parsed = thermite_syntax::parse(SOURCE);
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
        error.to_string().contains("aliases exclusive root"),
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
