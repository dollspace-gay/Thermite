use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::ast::{PrimType, Type};
use thermite_syntax::Item;
use thermite_tv::obligation::{
    loop_entry_obligation, loop_exit_obligation, loop_preservation_obligation,
    loop_result_obligation, LoopObligationFrame, LoopParamDecl,
};
use thermite_tv::{loop_ref_obligations, BodyRefCtx, NamedRecordFrame, RecordFieldFrame};

const SOURCE: &str = r#"
struct LoopInner { total: u64, guard: u64 }
struct LoopState { cursor: u64, inner: LoopInner, tag: u64 }
fn record_loop(limit: u64, guard: u64, tag: u64) -> LoopState
  req limit <= 100
  ens result.cursor == limit
  ens result.inner.total == limit
  ens result.inner.guard == guard
  ens result.tag == tag
  fx pure
{
  let mut state: LoopState = LoopState {
    cursor: 0,
    inner: LoopInner { total: 0, guard: guard },
    tag: tag,
  };
  while state.cursor < limit
    inv state.cursor <= limit
    inv state.inner.total == state.cursor
    inv state.inner.guard == guard
    inv state.tag == tag
    dec limit - state.cursor
  {
    state.inner.total = state.cursor + 1;
    state.cursor = state.cursor + 1;
  }
  state
}
"#;

const DEFINITIONS: &str = r#"
pub struct LoopInner {
    pub total: u64,
    pub guard: u64,
}
pub struct LoopState {
    pub cursor: u64,
    pub inner: LoopInner,
    pub tag: u64,
}
"#;

fn records() -> Vec<NamedRecordFrame> {
    vec![
        NamedRecordFrame::new(
            "LoopInner",
            vec![
                RecordFieldFrame::typed("total", Type::Prim(PrimType::U64)),
                RecordFieldFrame::typed("guard", Type::Prim(PrimType::U64)),
            ],
        ),
        NamedRecordFrame::new(
            "LoopState",
            vec![
                RecordFieldFrame::typed("cursor", Type::Prim(PrimType::U64)),
                RecordFieldFrame::typed("inner", Type::Named("LoopInner".to_string())),
                RecordFieldFrame::typed("tag", Type::Prim(PrimType::U64)),
            ],
        ),
    ]
}

fn body() -> thermite_syntax::Block {
    let parsed = thermite_syntax::parse(SOURCE);
    assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
    parsed
        .program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fn(function) => function.body.clone(),
            _ => None,
        })
        .expect("record loop body")
}

fn frame() -> LoopObligationFrame {
    LoopObligationFrame {
        spec_defs: vec![DEFINITIONS.to_string()],
        inputs: vec![
            LoopParamDecl::new("limit", "u64"),
            LoopParamDecl::new("guard", "u64"),
            LoopParamDecl::new("tag", "u64"),
        ],
        cells: vec![LoopParamDecl::new("state", "LoopState")],
        req: Some("limit <= 100".to_string()),
        ret_type: Some("LoopState".to_string()),
        named_records: records(),
        ..Default::default()
    }
}

fn faithful_step() -> &'static str {
    r#"    let mut state = state;
    state.inner.total = state.cursor + 1;
    state.cursor = state.cursor + 1;
    state
"#
}

fn faithful_result() -> &'static str {
    r#"    let mut state: LoopState = LoopState {
        cursor: 0,
        inner: LoopInner { total: 0, guard: guard },
        tag: tag,
    };
    while state.cursor < limit
        invariant
            state.cursor <= limit,
            state.inner.total == state.cursor,
            state.inner.guard == guard,
            state.tag == tag,
            limit <= 100,
        decreases limit - state.cursor,
    {
        state.inner.total = state.cursor + 1;
        state.cursor = state.cursor + 1;
    }
    state
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
        eprintln!("SKIP: verus unavailable; record-loop TV `{name}` not discharged");
        return;
    };
    let path = std::env::temp_dir().join(format!(
        "thermite_record_loop_tv_{}_{}.rs",
        std::process::id(),
        name
    ));
    std::fs::write(&path, program).expect("write record-loop obligation");
    let output = Command::new(verus)
        .arg(&path)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run verus for record-loop TV");
    let _ = std::fs::remove_file(&path);
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        output.status.success(),
        should_pass,
        "unexpected record-loop verdict for `{name}`:\n{combined}\n--- program ---\n{program}"
    );
}

#[test]
fn nested_record_loop_reference_is_exact_and_faithful() {
    let body = body();
    let context = BodyRefCtx::default()
        .with_named_records(records())
        .with_constructor_records(records());
    let obligations =
        loop_ref_obligations(&body, &context).expect("record loop is in the exact subset");
    assert_eq!(obligations.cells, ["state"]);
    assert!(
        obligations.step_cells[0].contains("LoopState"),
        "{:?}",
        obligations.step_cells
    );
    assert!(
        obligations.step_cells[0].contains("guard: state.inner.guard"),
        "{:?}",
        obligations.step_cells
    );
    assert_eq!(
        obligations.exit_result_pred.as_deref(),
        Some("(result.cursor <= limit) && (result.inner.total == result.cursor) && (result.inner.guard == guard) && (result.tag == tag) && (!((result.cursor < limit)))")
    );

    assert_verus(
        "entry_faithful",
        &loop_entry_obligation(&body, &frame()).expect("entry obligation"),
        true,
    );
    assert_verus(
        "step_faithful",
        &loop_preservation_obligation(&body, faithful_step(), &frame())
            .expect("preservation obligation"),
        true,
    );
    assert_verus(
        "exit_faithful",
        &loop_exit_obligation(&body, "state.cursor == limit", &frame()).expect("exit obligation"),
        true,
    );
    assert_verus(
        "result_faithful",
        &loop_result_obligation(&body, faithful_result(), &frame()).expect("result obligation"),
        true,
    );
}

#[test]
fn record_step_wrong_nested_collateral_and_order_mutants_fail() {
    let body = body();
    for (name, production) in [
        (
            "wrong_nested_value",
            faithful_step().replace("state.cursor + 1", "state.cursor"),
        ),
        (
            "collateral_guard",
            faithful_step().replace(
                "state.inner.total = state.cursor + 1;",
                "state.inner.guard = 0;\n    state.inner.total = state.cursor + 1;",
            ),
        ),
        (
            "reordered_dependent_writes",
            faithful_step().replace(
                "state.inner.total = state.cursor + 1;\n    state.cursor = state.cursor + 1;",
                "state.cursor = state.cursor + 1;\n    state.inner.total = state.cursor + 1;",
            ),
        ),
    ] {
        let obligation = loop_preservation_obligation(&body, &production, &frame())
            .expect("mutant preservation obligation");
        assert_verus(name, &obligation, false);
    }
}

#[test]
fn full_loop_dropped_loop_wrong_tail_and_missing_frame_mutants_fail() {
    let body = body();
    let dropped = r#"    let state: LoopState = LoopState {
        cursor: 0,
        inner: LoopInner { total: 0, guard: guard },
        tag: tag,
    };
    state
"#;
    assert_verus(
        "dropped_loop",
        &loop_result_obligation(&body, dropped, &frame()).expect("dropped-loop obligation"),
        false,
    );

    let wrong_tail = faithful_result().replace(
        "    state\n",
        "    LoopState { cursor: state.cursor, inner: state.inner, tag: 0 }\n",
    );
    assert_verus(
        "wrong_tail",
        &loop_result_obligation(&body, &wrong_tail, &frame()).expect("wrong-tail obligation"),
        false,
    );

    let missing_frame = faithful_result().replace("            state.tag == tag,\n", "");
    assert_verus(
        "missing_collateral_invariant",
        &loop_result_obligation(&body, &missing_frame, &frame()).expect("frame mutant"),
        false,
    );
}
