use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::Item;
use thermite_tv::obligation::{
    body_equivalence_obligation, equivalence_obligation, exec_equivalence_obligation,
    BodyObligationFrame, BodyParamDecl, ExecObligationFrame, ExecParamDecl, ObligationFrame,
    ParamDecl, StateViewDecl, StateViewKind,
};
use thermite_tv::{MutableRecordFrame, RecordFieldFrame};

const SOURCE: &str = r#"
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
"#;

const DEPENDENT_SOURCE: &str = r#"
struct Pair { first: u64, second: u64 }
fn chain(state: &mut Pair, next: u64) -> ()
  req true
  ens final(state).first == next
  ens final(state).second == next
  fx pure
{
  state.first = next;
  state.second = state.first;
}
"#;

fn source_body() -> thermite_syntax::Block {
    let parsed = thermite_syntax::parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected advance function");
    };
    function.body.clone().expect("advance has a body")
}

fn frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![
            "pub struct State {\n    pub generation: u64,\n    pub occupied: bool,\n}".to_string(),
        ],
        params: vec![
            BodyParamDecl::new("state", "&mut State"),
            BodyParamDecl::new("next", "u64"),
        ],
        ret_type: "bool".to_string(),
        req: Some("next > old(state).generation".to_string()),
        mutable_records: vec![MutableRecordFrame::new(
            "state",
            vec![
                RecordFieldFrame::new("generation", false),
                RecordFieldFrame::new("occupied", false),
            ],
        )],
        ..Default::default()
    }
}

fn dependent_body() -> thermite_syntax::Block {
    let parsed = thermite_syntax::parse(DEPENDENT_SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected chain function");
    };
    function.body.clone().expect("chain has a body")
}

fn dependent_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![
            "pub struct Pair {\n    pub first: u64,\n    pub second: u64,\n}".to_string(),
        ],
        params: vec![
            BodyParamDecl::new("state", "&mut Pair"),
            BodyParamDecl::new("next", "u64"),
        ],
        ret_type: "()".to_string(),
        result_is_unit: true,
        mutable_records: vec![MutableRecordFrame::new(
            "state",
            vec![
                RecordFieldFrame::new("first", false),
                RecordFieldFrame::new("second", false),
            ],
        )],
        ..Default::default()
    }
}

fn obligation(production: &str) -> String {
    body_equivalence_obligation(&source_body(), production, &frame())
        .expect("named-record body obligation must build")
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

fn discharge(name: &str, program: &str) -> Option<(bool, String)> {
    let temp = std::env::temp_dir();
    let stem = format!(
        "thermite_named_record_lifecycle_tv_{}_{}",
        std::process::id(),
        name
    );
    let path = temp.join(format!("{stem}.rs"));
    std::fs::write(&path, program).expect("write named-record TV obligation");
    let output = Command::new(verus_bin()?)
        .arg(&path)
        .current_dir(&temp)
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success(), combined))
}

#[test]
fn obligation_observes_result_changed_field_and_untouched_field() {
    let program = obligation(
        "    let previous: bool = state.occupied;\n    state.generation = next;\n    previous\n",
    );
    assert!(
        program.contains("result == old(state).occupied"),
        "{program}"
    );
    assert!(
        program.contains("final(state).generation == next"),
        "{program}"
    );
    assert!(
        program.contains("final(state).occupied == old(state).occupied"),
        "{program}"
    );
    match discharge("faithful", &program) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "faithful named-record lifecycle did not verify:\n{output}\n{program}"
        ),
        None => eprintln!("SKIP: verus unavailable; obligation shape still checked"),
    }
}

#[test]
fn dropped_wrong_and_collateral_field_mutants_fail() {
    for (name, production) in [
        (
            "dropped",
            "    let previous: bool = state.occupied;\n    previous\n",
        ),
        (
            "wrong_field",
            "    let previous: bool = state.occupied;\n    state.occupied = false;\n    previous\n",
        ),
        (
            "collateral",
            "    let previous: bool = state.occupied;\n    state.generation = next;\n    state.occupied = false;\n    previous\n",
        ),
    ] {
        let program = obligation(production);
        match discharge(name, &program) {
            Some((success, output)) => assert!(
                !success && !output.contains("verified, 0 errors"),
                "{name} mutant escaped the complete field frame:\n{output}\n{program}"
            ),
            None => eprintln!("SKIP: verus unavailable; {name} mutant obligation still formed"),
        }
    }
}

#[test]
fn wrong_value_and_reordered_dependent_write_mutants_fail() {
    let faithful = body_equivalence_obligation(
        &dependent_body(),
        "    state.first = next;\n    state.second = state.first;\n",
        &dependent_frame(),
    )
    .expect("dependent lifecycle obligation must build");
    match discharge("dependent_faithful", &faithful) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "faithful dependent lifecycle did not verify:\n{output}\n{faithful}"
        ),
        None => eprintln!("SKIP: verus unavailable; dependent obligation shape still checked"),
    }

    for (name, production) in [
        (
            "wrong_value",
            "    state.first = 0;\n    state.second = state.first;\n",
        ),
        (
            "reordered",
            "    state.second = state.first;\n    state.first = next;\n",
        ),
    ] {
        let program =
            body_equivalence_obligation(&dependent_body(), production, &dependent_frame())
                .expect("dependent mutant obligation must build");
        match discharge(name, &program) {
            Some((success, output)) => assert!(
                !success && !output.contains("verified, 0 errors"),
                "{name} mutant escaped exact ordered state threading:\n{output}\n{program}"
            ),
            None => eprintln!("SKIP: verus unavailable; {name} obligation still formed"),
        }
    }
}

#[test]
fn old_final_selector_swap_fails_contract_equivalence() {
    let parsed = thermite_syntax::parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected advance function");
    };
    let clause = &function.contract.ens[1].expr;
    let frame = ObligationFrame {
        spec_defs: vec![
            "pub struct State {\n    pub generation: u64,\n    pub occupied: bool,\n}".to_string(),
        ],
        params: vec![
            ParamDecl::new("next", "u64"),
            ParamDecl::new("old_state", "State"),
            ParamDecl::new("final_state", "State"),
        ],
        state_views: vec![
            StateViewDecl::new(StateViewKind::Old, "state", "old_state", false),
            StateViewDecl::new(StateViewKind::Final, "state", "final_state", false),
        ],
        ..Default::default()
    };
    let faithful = equivalence_obligation(clause, "final(state).generation == next", &frame)
        .expect("named-record contract obligation must build");
    match discharge("contract_selector_faithful", &faithful) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "faithful selector obligation did not verify:\n{output}\n{faithful}"
        ),
        None => eprintln!("SKIP: verus unavailable; contract selector shape still checked"),
    }

    let swapped = equivalence_obligation(clause, "old(state).generation == next", &frame)
        .expect("selector mutant obligation must build");
    match discharge("contract_selector_swapped", &swapped) {
        Some((success, output)) => assert!(
            !success && !output.contains("verified, 0 errors"),
            "old/final selector mutant escaped arbitrary snapshots:\n{output}\n{swapped}"
        ),
        None => eprintln!("SKIP: verus unavailable; selector mutant obligation still formed"),
    }
}

#[test]
fn struct_constructor_expression_is_faithful_and_field_sensitive() {
    let parsed = thermite_syntax::parse(
        "struct State { generation: u64, occupied: bool }\n\
         fn state_new(generation: u64, occupied: bool) -> State\n\
         req true ens result.generation == generation ens result.occupied == occupied fx pure\n\
         { State { generation: generation, occupied: occupied } }",
    );
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected state_new function");
    };
    let tail = function
        .body
        .as_ref()
        .and_then(|body| body.tail.as_deref())
        .expect("constructor has a tail");
    let frame = ExecObligationFrame {
        spec_defs: vec![
            "pub struct State {\n    pub generation: u64,\n    pub occupied: bool,\n}".to_string(),
        ],
        params: vec![
            ExecParamDecl::new("generation", "u64"),
            ExecParamDecl::new("occupied", "bool"),
        ],
        ret_type: "State".to_string(),
        ..Default::default()
    };
    let faithful = exec_equivalence_obligation(
        tail,
        "State { generation: generation, occupied: occupied }",
        &frame,
    )
    .expect("struct constructor exec obligation must build");
    match discharge("struct_constructor_faithful", &faithful) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "faithful constructor did not verify:\n{output}\n{faithful}"
        ),
        None => eprintln!("SKIP: verus unavailable; constructor obligation still formed"),
    }

    let wrong =
        exec_equivalence_obligation(tail, "State { generation: 0, occupied: occupied }", &frame)
            .expect("wrong constructor obligation must build");
    match discharge("struct_constructor_wrong_field", &wrong) {
        Some((success, output)) => assert!(
            !success && !output.contains("verified, 0 errors"),
            "wrong constructor escaped structural equality:\n{output}\n{wrong}"
        ),
        None => eprintln!("SKIP: verus unavailable; constructor mutant still formed"),
    }
}
