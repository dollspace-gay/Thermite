use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::Item;
use thermite_tv::obligation::{
    body_equivalence_obligation, equivalence_obligation, exec_equivalence_obligation,
    BodyObligationFrame, BodyParamDecl, ExecObligationFrame, ExecParamDecl, ObligationFrame,
    ParamDecl, StateViewDecl, StateViewKind,
};

const SOURCE: &str = "const SLOTS: usize = 4;\n\
fn replace(slots: [u64; SLOTS], at: usize, value: u64) -> [u64; SLOTS]\n\
req at < SLOTS\n\
ens result[at] == value\n\
fx pure\n\
{\n\
  let mut updated: [u64; SLOTS] = slots;\n\
  updated[at] = value;\n\
  updated\n\
}\n";

fn source_body() -> thermite_syntax::Block {
    let parsed = thermite_syntax::parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected replace function");
    };
    function
        .body
        .clone()
        .expect("replace has an in-language body")
}

fn frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec!["pub const SLOTS: usize = 4;".to_string()],
        params: vec![
            BodyParamDecl::new("slots", "[u64; SLOTS]"),
            BodyParamDecl::new("at", "usize"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "[u64; SLOTS]".to_string(),
        req: Some("at < SLOTS".to_string()),
        fixed_array_params: vec!["slots".to_string()],
        result_is_fixed_array: true,
        ..Default::default()
    }
}

fn obligation(production: &str) -> String {
    body_equivalence_obligation(&source_body(), production, &frame())
        .expect("fixed-array body obligation must build")
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
    let stem = format!("thermite_fixed_array_tv_{}_{}", std::process::id(), name);
    let path = temp.join(format!("{stem}.rs"));
    std::fs::write(&path, program).expect("write fixed-array TV obligation");
    let output = Command::new(verus_bin()?)
        .arg(&path)
        .current_dir(&temp)
        .output();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(temp.join(stem));
    let output = output.ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success(), text))
}

fn assert_verus(name: &str, program: &str, should_pass: bool) {
    let Some((passed, output)) = discharge(name, program) else {
        eprintln!("SKIP: verus unavailable; fixed-array TV `{name}` was not discharged");
        return;
    };
    assert_eq!(
        passed, should_pass,
        "unexpected fixed-array TV verdict for `{name}`:\n{output}\n--- program ---\n{program}"
    );
    if !should_pass {
        assert!(
            output.contains("postcondition not satisfied") || output.contains("assertion failed"),
            "negative fixed-array TV must fail at the extensional result postcondition or \
             predicate-equivalence assertion:\n{output}"
        );
    }
}

#[test]
fn fixed_array_reference_is_an_exact_extensional_update() {
    let program = obligation(
        "    let mut updated: [u64; SLOTS] = slots;\n\
         updated[at] = value;\n\
         updated",
    );
    assert!(
        program.contains("result@ == (vstd::array::spec_array_update(slots, at as int, value))@"),
        "{program}"
    );
    assert_verus("faithful", &program, true);
}

#[test]
fn fixed_array_tv_rejects_wrong_index_and_wrong_value() {
    let wrong_index = obligation(
        "    let mut updated: [u64; SLOTS] = slots;\n\
         updated[0] = value;\n\
         updated",
    );
    assert_verus("wrong_index", &wrong_index, false);

    let wrong_value = obligation(
        "    let mut updated: [u64; SLOTS] = slots;\n\
         updated[at] = 0;\n\
         updated",
    );
    assert_verus("wrong_value", &wrong_value, false);
}

#[test]
fn fixed_array_read_expression_matches_the_native_index() {
    let parsed = thermite_syntax::parse(SOURCE);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected replace function");
    };
    let index = match &function.body.as_ref().unwrap().stmts[1] {
        thermite_syntax::Stmt::Assign { target, .. } => target,
        other => panic!("expected indexed assignment, got {other:?}"),
    };
    let frame = ExecObligationFrame {
        spec_defs: vec!["pub const SLOTS: usize = 4;".to_string()],
        params: vec![
            ExecParamDecl::new("updated", "[u64; SLOTS]"),
            ExecParamDecl::new("at", "usize"),
        ],
        ret_type: "u64".to_string(),
        req: Some("at < SLOTS".to_string()),
        fixed_array_params: vec!["updated".to_string()],
        ..Default::default()
    };
    let program = exec_equivalence_obligation(index, "updated[at]", &frame)
        .expect("array-read exec obligation must build");
    assert_verus("array_read", &program, true);
}

#[test]
fn fixed_array_length_is_the_finite_view_length() {
    let source = "const SLOTS: usize = 4;\n\
fn array_len(slots: [u64; SLOTS]) -> usize\n\
req true\n\
ens result == slots.len()\n\
fx pure\n\
{ slots.len() }\n";
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected array_len function");
    };
    let tail = function.body.as_ref().unwrap().tail.as_ref().unwrap();
    let exec_frame = ExecObligationFrame {
        spec_defs: vec!["pub const SLOTS: usize = 4;".to_string()],
        params: vec![ExecParamDecl::new("slots", "[u64; SLOTS]")],
        ret_type: "usize".to_string(),
        fixed_array_params: vec!["slots".to_string()],
        ..Default::default()
    };
    let exec_program = exec_equivalence_obligation(tail, "slots.len()", &exec_frame)
        .expect("array length exec obligation must build");
    assert!(
        exec_program.contains("result == (slots@.len() as usize)"),
        "{exec_program}"
    );
    assert_verus("array_len_exec", &exec_program, true);
    let wrong_exec = exec_equivalence_obligation(tail, "0", &exec_frame)
        .expect("wrong array length obligation must still build");
    assert_verus("array_len_wrong", &wrong_exec, false);

    let contract_frame = ObligationFrame {
        spec_defs: vec!["pub const SLOTS: usize = 4;".to_string()],
        params: vec![
            ParamDecl::new("slots", "[u64; SLOTS]"),
            ParamDecl::new("result", "usize"),
        ],
        fixed_array_params: vec!["slots".to_string()],
        ..Default::default()
    };
    let contract_program = equivalence_obligation(
        &function.contract.ens[0].expr,
        "result == slots.len()",
        &contract_frame,
    )
    .expect("array length contract obligation must build");
    assert!(contract_program.contains("(slots)@.len() as usize"));
    assert_verus("array_len_contract", &contract_program, true);
}

#[test]
fn named_spec_call_preserves_a_borrowed_fixed_array() {
    let source = "const SLOTS: usize = 4;\n\
spec fn first_is_zero(slots: &[u64; SLOTS]) -> bool\n\
dec 0\n\
{ slots[0] == 0 }\n\
fn observe(slots: &[u64; SLOTS]) -> bool\n\
req true\n\
ens result == first_is_zero(slots)\n\
fx pure\n\
{ slots[0] == 0 }\n";
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[2] else {
        panic!("expected observe function");
    };
    let frame = ObligationFrame {
        spec_defs: vec![
            "pub const SLOTS: usize = 4;".to_string(),
            "pub open spec fn first_is_zero(slots: &[u64; SLOTS]) -> bool { \
             slots@[0] == 0 }"
                .to_string(),
        ],
        params: vec![
            ParamDecl::new("slots", "&[u64; SLOTS]"),
            ParamDecl::new("result", "bool"),
        ],
        fixed_array_params: vec!["slots".to_string()],
        spec_call_slice_args: vec![("first_is_zero".to_string(), vec![])],
        ..Default::default()
    };
    let program = equivalence_obligation(
        &function.contract.ens[0].expr,
        "result == first_is_zero(slots)",
        &frame,
    )
    .expect("borrowed-array named-call obligation must build");
    assert!(program.contains("first_is_zero(slots)"), "{program}");
    assert!(!program.contains("first_is_zero(slots@)"), "{program}");
    assert_verus("borrowed_array_named_spec", &program, true);

    let wrong = equivalence_obligation(
        &function.contract.ens[0].expr,
        "result != first_is_zero(slots)",
        &frame,
    )
    .expect("wrong borrowed-array named-call obligation must still build");
    assert_verus("borrowed_array_named_spec_wrong", &wrong, false);
}

#[test]
fn fixed_array_equality_is_a_verified_extensional_scan() {
    let source = "const SLOTS: usize = 4;\n\
fn arrays_equal(left: [u64; SLOTS], right: [u64; SLOTS]) -> bool\n\
req true\n\
ens result == left.array_eq(right)\n\
fx pure\n\
{ left.array_eq(right) }\n";
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[1] else {
        panic!("expected arrays_equal function");
    };
    let tail = function.body.as_ref().unwrap().tail.as_ref().unwrap();
    // Deliberately independent from thermite-lower: this crate may not depend on
    // production lowering. Forge feeds the exact generated helper into the same
    // obligation shape.
    let production = "__thermite_array_eq_u64(&(left), &(right))";
    let helper = r#"
pub fn __thermite_array_eq_u64(
    left: &[u64; SLOTS],
    right: &[u64; SLOTS],
) -> (result: bool)
    ensures result <==> left@ =~= right@,
{
    let mut i: usize = 0;
    while i < SLOTS
        invariant
            i <= SLOTS,
            forall|j: int| 0 <= j < i ==> left@[j] == right@[j],
        decreases SLOTS - i,
    {
        if left[i] != right[i] {
            assert(left@[i as int] != right@[i as int]);
            return false;
        }
        i = i + 1;
    }
    assert(left@ =~= right@);
    true
}

"#
    .to_string();
    let exec_frame = ExecObligationFrame {
        spec_defs: vec!["pub const SLOTS: usize = 4;".to_string(), helper.clone()],
        params: vec![
            ExecParamDecl::new("left", "[u64; SLOTS]"),
            ExecParamDecl::new("right", "[u64; SLOTS]"),
        ],
        ret_type: "bool".to_string(),
        fixed_array_params: vec!["left".to_string(), "right".to_string()],
        ..Default::default()
    };
    let exec_program = exec_equivalence_obligation(tail, production, &exec_frame)
        .expect("array equality exec obligation must build");
    assert!(
        exec_program.contains("result == ((left)@ =~= (right)@)"),
        "{exec_program}"
    );
    assert_verus("array_eq_exec", &exec_program, true);

    let wrong_exec = exec_equivalence_obligation(tail, "true", &exec_frame)
        .expect("wrong equality production must still build");
    assert_verus("array_eq_wrong", &wrong_exec, false);

    let contract_frame = ObligationFrame {
        spec_defs: vec!["pub const SLOTS: usize = 4;".to_string()],
        params: vec![
            ParamDecl::new("left", "[u64; SLOTS]"),
            ParamDecl::new("right", "[u64; SLOTS]"),
            ParamDecl::new("result", "bool"),
        ],
        fixed_array_params: vec!["left".to_string(), "right".to_string()],
        ..Default::default()
    };
    let contract_program = equivalence_obligation(
        &function.contract.ens[0].expr,
        "result == ((left)@ =~= (right)@)",
        &contract_frame,
    )
    .expect("array equality contract obligation must build");
    assert_verus("array_eq_contract", &contract_program, true);
}

#[test]
fn exclusive_aggregate_slice_write_has_an_exact_post_state() {
    let source = "fn write_pair(data: &mut [[u64; 2]], at: usize, value: u64) -> u64\n\
req at < data.len()\n\
ens result == value\n\
ens final(data)[at][0] == value\n\
fx platform(memory)\n\
{ data[at] = [value, value]; value }\n";
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[0] else {
        panic!("expected write_pair function");
    };
    let body = function.body.as_ref().expect("write_pair body");
    let frame = BodyObligationFrame {
        params: vec![
            BodyParamDecl::new("data", "&mut [[u64; 2]]"),
            BodyParamDecl::new("at", "usize"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "u64".to_string(),
        req: Some("at < data.len()".to_string()),
        slice_params: vec!["data".to_string()],
        mutable_indexed_params: vec!["data".to_string()],
        ..Default::default()
    };

    let faithful =
        body_equivalence_obligation(body, "    data[at] = [value, value];\n    value\n", &frame)
            .expect("aggregate-slice body obligation must build");
    assert!(
        faithful.contains("final(data)@ == (old(data)@).update((at) as int, [value, value])"),
        "{faithful}"
    );
    assert_verus("aggregate_slice_faithful", &faithful, true);

    let wrong_value =
        body_equivalence_obligation(body, "    data[at] = [0, value];\n    value\n", &frame)
            .expect("wrong aggregate-slice production must still build");
    assert_verus("aggregate_slice_wrong_value", &wrong_value, false);

    let dropped_write = body_equivalence_obligation(body, "    value\n", &frame)
        .expect("dropped aggregate-slice write must still build");
    assert_verus("aggregate_slice_dropped_write", &dropped_write, false);
}

#[test]
fn contract_tv_quantifies_mutable_slice_post_state_independently() {
    let source = "fn observe(data: &mut [[u64; 2]], at: usize, value: u64) -> u64\n\
req at < data.len()\n\
ens final(data)[at][0] == value\n\
fx platform(memory)\n\
{ value }\n";
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    let Item::Fn(function) = &parsed.program.items[0] else {
        panic!("expected observe function");
    };
    let frame = ObligationFrame {
        params: vec![
            ParamDecl::new("data", "&[[u64; 2]]"),
            ParamDecl::new("at", "usize"),
            ParamDecl::new("value", "u64"),
            ParamDecl::new("result", "u64"),
            ParamDecl::new("final_data", "Seq<[u64; 2]>"),
        ],
        req: Some("at < data.len()".to_string()),
        seq_params: vec!["final_data".to_string()],
        state_views: vec![StateViewDecl::new(
            StateViewKind::Final,
            "data",
            "final_data",
            true,
        )],
        ..Default::default()
    };
    let clause = &function.contract.ens[0].expr;
    let faithful = equivalence_obligation(
        clause,
        "final(data)@[at as int]@[0 as int] == value",
        &frame,
    )
    .expect("state-view contract obligation must build");
    assert!(faithful.contains("final_data[at as int]@[0 as int]"));
    assert!(!faithful.contains("final(data)"));
    assert_verus("aggregate_contract_final", &faithful, true);

    let current_state =
        equivalence_obligation(clause, "data@[at as int]@[0 as int] == value", &frame)
            .expect("current-state mutant obligation must build");
    assert_verus("aggregate_contract_current_mutant", &current_state, false);
}
