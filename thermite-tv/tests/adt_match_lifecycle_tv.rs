use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::ast::{PrimType, Type};
use thermite_syntax::Item;
use thermite_tv::obligation::{
    body_equivalence_obligation, equivalence_obligation, BodyObligationFrame, BodyParamDecl,
    ObligationFrame, ParamDecl,
};
use thermite_tv::{EnumVariantFrame, EnumVariantShapeFrame, NamedRecordFrame, RecordFieldFrame};

const SOURCE: &str = r#"
struct State { generation: u64, owner: u64 }
enum Event { Tick(u64), Idle }
enum Action { Store { generation: u64, value: u64 }, Noop }
fn step(state: State, event: Event) -> (State, Action)
  req state.generation < 100
  ens result.0.generation == state.generation + 1
  ens result.0.owner == state.owner
  fx pure
{
  match event {
    Event::Tick(value) => (
      State { generation: state.generation + 1, owner: state.owner },
      Action::Store { generation: state.generation + 1, value: value },
    ),
    Event::Idle => (
      State { generation: state.generation + 1, owner: state.owner },
      Action::Noop,
    ),
  }
}
"#;

const SHADOW_SOURCE: &str = r#"
struct State { generation: u64, owner: u64 }
enum Event { Tick(u64), Idle }
enum Action { Store { generation: u64, value: u64 }, Noop }
fn shadow(state: State, event: Event) -> (State, Action)
  req state.generation < 100
  ens result.0.generation == state.generation + 1
  fx pure
{
  let value: u64 = 99;
  match event {
    Event::Tick(value) => (
      State { generation: state.generation + 1, owner: state.owner },
      Action::Store { generation: state.generation + 1, value: value },
    ),
    Event::Idle => (
      State { generation: state.generation + 1, owner: state.owner },
      Action::Noop,
    ),
  }
}
"#;

const DEFINITIONS: &str = r#"
pub struct State {
    pub generation: u64,
    pub owner: u64,
}
pub enum Event {
    Tick(u64),
    Idle,
}
pub enum Action {
    Store { generation: u64, value: u64 },
    Noop,
}
"#;

fn function(source: &str) -> thermite_syntax::FnItem {
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) => Some(function.clone()),
            _ => None,
        })
        .expect("test source must contain a function")
}

fn frame() -> BodyObligationFrame {
    let state = NamedRecordFrame::new(
        "State",
        vec![
            RecordFieldFrame::typed("generation", Type::Prim(PrimType::U64)),
            RecordFieldFrame::typed("owner", Type::Prim(PrimType::U64)),
        ],
    );
    BodyObligationFrame {
        spec_defs: vec![DEFINITIONS.to_string()],
        params: vec![
            BodyParamDecl::new("state", "State"),
            BodyParamDecl::new("event", "Event"),
        ],
        ret_type: "(State, Action)".to_string(),
        req: Some("state.generation < 100".to_string()),
        constructor_records: vec![state],
        enum_variants: vec![
            EnumVariantFrame::new(
                "Event",
                "Tick",
                EnumVariantShapeFrame::Tuple(vec![Type::Prim(PrimType::U64)]),
            ),
            EnumVariantFrame::new("Event", "Idle", EnumVariantShapeFrame::Unit),
            EnumVariantFrame::new(
                "Action",
                "Store",
                EnumVariantShapeFrame::Struct(vec![
                    RecordFieldFrame::typed("generation", Type::Prim(PrimType::U64)),
                    RecordFieldFrame::typed("value", Type::Prim(PrimType::U64)),
                ]),
            ),
            EnumVariantFrame::new("Action", "Noop", EnumVariantShapeFrame::Unit),
        ],
        ..Default::default()
    }
}

fn faithful_production() -> &'static str {
    r#"
    match event {
        Event::Tick(value) => (
            State { generation: state.generation + 1, owner: state.owner },
            Action::Store { generation: state.generation + 1, value: value },
        ),
        Event::Idle => (
            State { generation: state.generation + 1, owner: state.owner },
            Action::Noop,
        ),
    }
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
        eprintln!("SKIP: verus unavailable; ADT-match TV `{name}` was not discharged");
        return;
    };
    let stem = format!("thermite_adt_match_tv_{}_{}", std::process::id(), name);
    let path = std::env::temp_dir().join(format!("{stem}.rs"));
    std::fs::write(&path, program).expect("write ADT-match TV obligation");
    let output = Command::new(verus)
        .arg(&path)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run verus for ADT-match TV");
    let _ = std::fs::remove_file(&path);
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        output.status.success(),
        should_pass,
        "unexpected ADT-match TV verdict for `{name}`:\n{combined}\n--- program ---\n{program}"
    );
}

fn body_obligation(source: &str, production: &str) -> String {
    let function = function(source);
    body_equivalence_obligation(
        function.body.as_ref().expect("function body"),
        production,
        &frame(),
    )
    .expect("ADT-match body obligation must build")
}

#[test]
fn user_adt_match_and_aggregate_result_are_l3_faithful() {
    let program = body_obligation(SOURCE, faithful_production());
    assert!(program.contains("Event::Tick(value)"), "{program}");
    assert!(program.contains("Action::Store"), "{program}");
    assert!(program.contains("as u64"), "{program}");
    assert_verus("body_faithful", &program, true);
}

#[test]
fn adt_arm_variant_payload_and_collateral_mutants_fail() {
    for (name, production) in [
        (
            "wrong_variant",
            faithful_production().replace(
                "Action::Store { generation: state.generation + 1, value: value }",
                "Action::Noop",
            ),
        ),
        (
            "wrong_payload",
            faithful_production().replace("value: value", "value: 0"),
        ),
        (
            "dropped_generation",
            faithful_production().replace(
                "State { generation: state.generation + 1, owner: state.owner }",
                "State { generation: state.generation, owner: state.owner }",
            ),
        ),
        (
            "collateral_owner",
            faithful_production().replace("owner: state.owner", "owner: 0"),
        ),
        (
            "idle_uses_store",
            faithful_production().replace(
                "Action::Noop,",
                "Action::Store { generation: state.generation + 1, value: 0 },",
            ),
        ),
    ] {
        assert_verus(name, &body_obligation(SOURCE, &production), false);
    }
}

#[test]
fn pattern_payload_binding_shadows_outer_state_binding() {
    let faithful = format!("    let value: u64 = 99;\n{}", faithful_production());
    assert_verus(
        "pattern_shadow_faithful",
        &body_obligation(SHADOW_SOURCE, &faithful),
        true,
    );
    let captured_outer = faithful.replace("value: value", "value: 99");
    assert_verus(
        "pattern_shadow_capture_mutant",
        &body_obligation(SHADOW_SOURCE, &captured_outer),
        false,
    );
}

#[test]
fn function_parameter_shadows_unqualified_variant_constructor() {
    let shadow = function(
        "enum Signal { Idle }\n\
         fn echo(Idle: u64) -> u64 req true ens result == Idle fx pure { Idle }\n",
    );
    let shadow_frame = BodyObligationFrame {
        spec_defs: vec!["pub enum Signal { Idle }".to_string()],
        params: vec![BodyParamDecl::new("Idle", "u64")],
        ret_type: "u64".to_string(),
        req: Some("true".to_string()),
        enum_variants: vec![EnumVariantFrame::new(
            "Signal",
            "Idle",
            EnumVariantShapeFrame::Unit,
        )],
        ..Default::default()
    };
    let program = body_equivalence_obligation(
        shadow.body.as_ref().expect("shadow body"),
        "    Idle\n",
        &shadow_frame,
    )
    .expect("shadow-aware ADT obligation must build");
    assert!(program.contains("ensures result == Idle"), "{program}");
    assert!(
        !program.contains("ensures result == Signal::Idle"),
        "{program}"
    );
    assert_verus("parameter_shadow_faithful", &program, true);
}

#[test]
fn contract_user_enum_match_is_independently_l3_checked() {
    let parsed = thermite_syntax::parse(
        "enum Event { Tick(u64), Idle }\n\
         fn observes(event: Event, observed: u64) -> bool req true \
         ens match event { Event::Tick(value) => value == observed, \
         Event::Idle => observed == 0, } fx pure { true }\n",
    );
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let function = parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) => Some(function),
            _ => None,
        })
        .expect("contract function");
    let clause = &function.contract.ens[0].expr;
    let contract_frame = ObligationFrame {
        spec_defs: vec!["pub enum Event { Tick(u64), Idle }".to_string()],
        params: vec![
            ParamDecl::new("event", "Event"),
            ParamDecl::new("observed", "u64"),
        ],
        enum_variants: vec![
            ("Tick".to_string(), "Event".to_string()),
            ("Idle".to_string(), "Event".to_string()),
        ],
        ..Default::default()
    };
    let faithful = equivalence_obligation(
        clause,
        "match event { Event::Tick(value) => value == observed, Event::Idle => observed == 0, }",
        &contract_frame,
    )
    .expect("user-enum contract obligation");
    assert_verus("contract_faithful", &faithful, true);

    let wrong = equivalence_obligation(
        clause,
        "match event { Event::Tick(value) => value != observed, Event::Idle => observed == 0, }",
        &contract_frame,
    )
    .expect("user-enum contract mutant obligation");
    assert_verus("contract_payload_mutant", &wrong, false);
}
