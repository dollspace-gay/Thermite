use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::ast::{ArrayLen, Block, PrimType, Type};
use thermite_syntax::Item;
use thermite_tv::obligation::{body_equivalence_obligation, BodyObligationFrame, BodyParamDecl};
use thermite_tv::{MutableCallEffectFrame, MutableIndexedFrame, SharedIndexedFrame};

const ARRAY_SOURCE: &str = r#"
const SLOTS: usize = 4;
fn write_array(data: &mut [u64; SLOTS], at: usize, value: u64) -> u64
  req at < SLOTS
  ens result == value
  ens final(data)[at] == value
  fx pure
{
  data[at] = value;
  data[at]
}
fn array_pipeline(data: &mut [u64; SLOTS], at: usize, value: u64) -> u64
  req at < SLOTS
  ens result == value
  ens final(data)[at] == value
  fx pure
{
  let observed: u64 = write_array(data, at, value);
  observed
}
"#;

const SLICE_SOURCE: &str = r#"
fn write_slice(data: &mut [u64], at: usize, value: u64) -> u64
  req at < data.len()
  ens result == value
  ens final(data)[at] == value
  fx pure
{
  data[at] = value;
  data[at]
}
fn slice_pipeline(data: &mut [u64], at: usize, value: u64) -> u64
  req at < data.len()
  ens result == value
  ens final(data)[at] == value
  fx pure
{
  let observed: u64 = write_slice(data, at, value);
  observed
}
"#;

const ARRAY_DEFINITION: &str = r#"
pub const SLOTS: usize = 4;
fn write_array(data: &mut [u64; SLOTS], at: usize, value: u64) -> (result: u64)
    requires at < SLOTS,
    ensures
        result == value,
        final(data)@ == (old(data)@).update(at as int, value),
{
    data[at] = value;
    data[at]
}
"#;

const SLICE_DEFINITION: &str = r#"
fn write_slice(data: &mut [u64], at: usize, value: u64) -> (result: u64)
    requires at < data.len(),
    ensures
        result == value,
        final(data)@ == (old(data)@).update(at as int, value),
{
    data[at] = value;
    data[at]
}
"#;

const MIXED_ARRAY_SOURCE: &str = r#"
const SLOTS: usize = 4;
fn copy_array(left: &mut [u64; SLOTS], right: &[u64; SLOTS], at: usize) -> u64
  req at < SLOTS
  ens result == right[at]
  ens final(left)[0] == right[at]
  fx pure
{
  left[0] = right[at];
  left[0]
}
fn mixed_array_pipeline(
  left: &mut [u64; SLOTS],
  right: &[u64; SLOTS],
  at: usize,
) -> u64
  req at < SLOTS
  ens result == right[at]
  ens final(left)[0] == right[at]
  fx pure
{
  let observed: u64 = copy_array(left, right, at);
  observed
}
fn current_peer_pipeline(
  left: &mut [u64; SLOTS],
  peer: &mut [u64; SLOTS],
  value: u64,
) -> u64
  req true
  ens result == value
  ens final(left)[0] == value
  ens final(peer)[1] == value
  fx pure
{
  peer[1] = value;
  let observed: u64 = copy_array(left, peer, 1);
  observed
}
"#;

const MIXED_SLICE_SOURCE: &str = r#"
fn copy_slice(left: &mut [u64], right: &[u64], at: usize) -> u64
  req at < left.len() && at < right.len()
  ens result == right[at]
  ens final(left)[0] == right[at]
  fx pure
{
  left[0] = right[at];
  left[0]
}
fn mixed_slice_pipeline(left: &mut [u64], right: &[u64], at: usize) -> u64
  req 0 < left.len() && at < left.len() && at < right.len()
  ens result == right[at]
  ens final(left)[0] == right[at]
  fx pure
{
  let observed: u64 = copy_slice(left, right, at);
  observed
}
"#;

const MIXED_ARRAY_DEFINITION: &str = r#"
pub const SLOTS: usize = 4;
fn copy_array(
    left: &mut [u64; SLOTS],
    right: &[u64; SLOTS],
    at: usize,
) -> (result: u64)
    requires at < SLOTS,
    ensures
        result == right@[at as int],
        final(left)@ == (old(left)@).update(0, right@[at as int]),
{
    left[0] = right[at];
    left[0]
}
"#;

const MIXED_SLICE_DEFINITION: &str = r#"
fn copy_slice(left: &mut [u64], right: &[u64], at: usize) -> (result: u64)
    requires 0 < left.len() && at < left.len() && at < right.len(),
    ensures
        result == right@[at as int],
        final(left)@ == (old(left)@).update(0, right@[at as int]),
{
    left[0] = right[at];
    left[0]
}
"#;

fn function_body(source: &str, name: &str) -> Block {
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

fn array_type() -> Type {
    Type::Array {
        elem: Box::new(Type::Prim(PrimType::U64)),
        len: ArrayLen::Const("SLOTS".to_string()),
    }
}

fn slice_type() -> Type {
    Type::Slice(Box::new(Type::Prim(PrimType::U64)))
}

fn array_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![ARRAY_DEFINITION.to_string()],
        params: vec![
            BodyParamDecl::new("data", "&mut [u64; SLOTS]"),
            BodyParamDecl::new("at", "usize"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "u64".to_string(),
        req: Some("at < SLOTS".to_string()),
        mutable_indexed_params: vec!["data".to_string()],
        fixed_array_params: vec!["data".to_string()],
        mutable_indexed: vec![MutableIndexedFrame::new("data", array_type())],
        mutable_call_effects: vec![MutableCallEffectFrame::new(
            "write_array",
            vec!["data".to_string(), "at".to_string(), "value".to_string()],
            Vec::new(),
            function_body(ARRAY_SOURCE, "write_array"),
        )
        .with_mutable_indexed(vec![MutableIndexedFrame::new("data", array_type())])],
        ..Default::default()
    }
}

fn slice_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![SLICE_DEFINITION.to_string()],
        params: vec![
            BodyParamDecl::new("data", "&mut [u64]"),
            BodyParamDecl::new("at", "usize"),
            BodyParamDecl::new("value", "u64"),
        ],
        ret_type: "u64".to_string(),
        req: Some("at < data.len()".to_string()),
        slice_params: vec!["data".to_string()],
        mutable_indexed_params: vec!["data".to_string()],
        mutable_indexed: vec![MutableIndexedFrame::new("data", slice_type())],
        mutable_call_effects: vec![MutableCallEffectFrame::new(
            "write_slice",
            vec!["data".to_string(), "at".to_string(), "value".to_string()],
            Vec::new(),
            function_body(SLICE_SOURCE, "write_slice"),
        )
        .with_mutable_indexed(vec![MutableIndexedFrame::new("data", slice_type())])],
        ..Default::default()
    }
}

fn mixed_array_frame(function: &str) -> BodyObligationFrame {
    let peer = function == "current_peer_pipeline";
    BodyObligationFrame {
        spec_defs: vec![MIXED_ARRAY_DEFINITION.to_string()],
        params: if peer {
            vec![
                BodyParamDecl::new("left", "&mut [u64; SLOTS]"),
                BodyParamDecl::new("peer", "&mut [u64; SLOTS]"),
                BodyParamDecl::new("value", "u64"),
            ]
        } else {
            vec![
                BodyParamDecl::new("left", "&mut [u64; SLOTS]"),
                BodyParamDecl::new("right", "&[u64; SLOTS]"),
                BodyParamDecl::new("at", "usize"),
            ]
        },
        ret_type: "u64".to_string(),
        req: (!peer).then(|| "at < SLOTS".to_string()),
        mutable_indexed_params: if peer {
            vec!["left".to_string(), "peer".to_string()]
        } else {
            vec!["left".to_string()]
        },
        fixed_array_params: if peer {
            vec!["left".to_string(), "peer".to_string()]
        } else {
            vec!["left".to_string(), "right".to_string()]
        },
        mutable_indexed: if peer {
            vec![
                MutableIndexedFrame::new("left", array_type()),
                MutableIndexedFrame::new("peer", array_type()),
            ]
        } else {
            vec![MutableIndexedFrame::new("left", array_type())]
        },
        shared_indexed: if peer {
            Vec::new()
        } else {
            vec![SharedIndexedFrame::new("right", array_type())]
        },
        mutable_call_effects: vec![MutableCallEffectFrame::new(
            "copy_array",
            vec!["left".to_string(), "right".to_string(), "at".to_string()],
            Vec::new(),
            function_body(MIXED_ARRAY_SOURCE, "copy_array"),
        )
        .with_mutable_indexed(vec![MutableIndexedFrame::new("left", array_type())])
        .with_shared_indexed(vec![SharedIndexedFrame::new("right", array_type())])],
        ..Default::default()
    }
}

fn mixed_slice_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        spec_defs: vec![MIXED_SLICE_DEFINITION.to_string()],
        params: vec![
            BodyParamDecl::new("left", "&mut [u64]"),
            BodyParamDecl::new("right", "&[u64]"),
            BodyParamDecl::new("at", "usize"),
        ],
        ret_type: "u64".to_string(),
        req: Some("0 < left.len() && at < left.len() && at < right.len()".to_string()),
        slice_params: vec!["left".to_string(), "right".to_string()],
        mutable_indexed_params: vec!["left".to_string()],
        mutable_indexed: vec![MutableIndexedFrame::new("left", slice_type())],
        shared_indexed: vec![SharedIndexedFrame::new("right", slice_type())],
        mutable_call_effects: vec![MutableCallEffectFrame::new(
            "copy_slice",
            vec!["left".to_string(), "right".to_string(), "at".to_string()],
            Vec::new(),
            function_body(MIXED_SLICE_SOURCE, "copy_slice"),
        )
        .with_mutable_indexed(vec![MutableIndexedFrame::new("left", slice_type())])
        .with_shared_indexed(vec![SharedIndexedFrame::new("right", slice_type())])],
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
        eprintln!("SKIP: verus unavailable; indexed-call TV `{name}` was not discharged");
        return;
    };
    let path = std::env::temp_dir().join(format!(
        "thermite_mutable_indexed_call_tv_{}_{}.rs",
        std::process::id(),
        name
    ));
    std::fs::write(&path, program).expect("write mutable-indexed call obligation");
    let output = Command::new(verus)
        .arg(&path)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run verus for mutable-indexed call TV");
    let _ = std::fs::remove_file(&path);
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    assert_eq!(
        output.status.success(),
        should_pass,
        "unexpected indexed-call TV verdict for `{name}`:\n{combined}\n--- program ---\n{program}"
    );
}

#[test]
fn fixed_array_call_result_and_complete_post_state_compose() {
    let source = function_body(ARRAY_SOURCE, "array_pipeline");
    let production = "    let observed: u64 = write_array(data, at, value);\n    observed\n";
    let obligation = body_equivalence_obligation(&source, production, &array_frame())
        .expect("fixed-array call-effect obligation");
    assert!(
        obligation.contains("final(data)@ == (old(data)@).update((at) as int, value)"),
        "{obligation}"
    );
    assert!(obligation.contains("result =="), "{obligation}");
    assert_verus("fixed_array_call", &obligation, true);

    let dropped = body_equivalence_obligation(&source, "    value\n", &array_frame())
        .expect("dropped fixed-array call obligation");
    assert_verus("fixed_array_dropped_call", &dropped, false);
}

#[test]
fn mutable_slice_call_result_and_complete_post_state_compose() {
    let source = function_body(SLICE_SOURCE, "slice_pipeline");
    let production = "    let observed: u64 = write_slice(data, at, value);\n    observed\n";
    let obligation = body_equivalence_obligation(&source, production, &slice_frame())
        .expect("mutable-slice call-effect obligation");
    assert!(
        obligation.contains("final(data)@ == (old(data)@).update((at) as int, value)"),
        "{obligation}"
    );
    assert_verus("mutable_slice_call", &obligation, true);
}

#[test]
fn indexed_alias_and_exact_type_mismatch_fail_closed() {
    let alias_source = r#"
fn touch_two(left: &mut [u64; SLOTS], right: &mut [u64; SLOTS], value: u64) -> ()
  req true
  ens true
  fx pure
{
  left[0] = value;
  right[1] = value;
}
fn alias(data: &mut [u64; SLOTS], value: u64) -> ()
  req true
  ens true
  fx pure
{
  touch_two(data, data, value);
}
"#;
    let body = function_body(alias_source, "alias");
    let mut frame = array_frame();
    frame.ret_type = "()".to_string();
    frame.result_is_unit = true;
    frame.params = vec![
        BodyParamDecl::new("data", "&mut [u64; SLOTS]"),
        BodyParamDecl::new("value", "u64"),
    ];
    frame.req = None;
    frame.mutable_call_effects = vec![MutableCallEffectFrame::new(
        "touch_two",
        vec!["left".to_string(), "right".to_string(), "value".to_string()],
        Vec::new(),
        function_body(alias_source, "touch_two"),
    )
    .with_mutable_indexed(vec![
        MutableIndexedFrame::new("left", array_type()),
        MutableIndexedFrame::new("right", array_type()),
    ])];
    let error = body_equivalence_obligation(&body, "", &frame)
        .expect_err("duplicate indexed exclusive actual must fail closed");
    assert!(
        error
            .to_string()
            .contains("aliases exclusive access paths `data` and `data`"),
        "{error}"
    );

    let mut mismatch = array_frame();
    mismatch.mutable_indexed = vec![MutableIndexedFrame::new(
        "data",
        Type::Array {
            elem: Box::new(Type::Prim(PrimType::U64)),
            len: ArrayLen::Literal {
                value: 8,
                raw: "8".to_string(),
            },
        },
    )];
    let error = body_equivalence_obligation(
        &function_body(ARRAY_SOURCE, "array_pipeline"),
        "",
        &mismatch,
    )
    .expect_err("indexed pointee mismatch must fail closed");
    assert!(
        error.to_string().contains("pointee-type mismatch"),
        "{error}"
    );
}

#[test]
fn shared_fixed_array_snapshot_and_current_mutable_peer_compose() {
    let source = function_body(MIXED_ARRAY_SOURCE, "mixed_array_pipeline");
    let production = "    let observed: u64 = copy_array(left, right, at);\n    observed\n";
    let obligation = body_equivalence_obligation(
        &source,
        production,
        &mixed_array_frame("mixed_array_pipeline"),
    )
    .expect("mixed fixed-array call-effect obligation");
    assert!(obligation.contains("(right@)[(at) as int]"), "{obligation}");
    assert!(obligation.contains("final(left)@ =="), "{obligation}");
    assert_verus("shared_fixed_array_call", &obligation, true);

    let peer_source = function_body(MIXED_ARRAY_SOURCE, "current_peer_pipeline");
    let peer_production =
        "    peer[1] = value;\n    let observed: u64 = copy_array(left, peer, 1);\n    observed\n";
    let peer_obligation = body_equivalence_obligation(
        &peer_source,
        peer_production,
        &mixed_array_frame("current_peer_pipeline"),
    )
    .expect("current mutable-peer snapshot obligation");
    assert!(
        peer_obligation.contains("(old(peer)@).update(1, value)"),
        "{peer_obligation}"
    );
    assert_verus("current_mutable_peer_snapshot", &peer_obligation, true);
}

#[test]
fn shared_mutable_slice_snapshot_composes() {
    let source = function_body(MIXED_SLICE_SOURCE, "mixed_slice_pipeline");
    let production = "    let observed: u64 = copy_slice(left, right, at);\n    observed\n";
    let obligation = body_equivalence_obligation(&source, production, &mixed_slice_frame())
        .expect("mixed mutable-slice call-effect obligation");
    assert!(obligation.contains("(right@)[(at) as int]"), "{obligation}");
    assert_verus("shared_mutable_slice_call", &obligation, true);
}

#[test]
fn shared_indexed_overlap_and_type_mismatch_fail_closed() {
    let alias_source = r#"
const SLOTS: usize = 4;
fn copy_array(left: &mut [u64; SLOTS], right: &[u64; SLOTS], at: usize) -> u64
  req at < SLOTS
  ens result == right[at]
  fx pure
{
  left[0] = right[at];
  left[0]
}
fn alias(data: &mut [u64; SLOTS], at: usize) -> u64
  req at < SLOTS
  ens true
  fx pure
{
  let observed: u64 = copy_array(data, data, at);
  observed
}
"#;
    let mut alias_frame = mixed_array_frame("mixed_array_pipeline");
    alias_frame.params = vec![
        BodyParamDecl::new("data", "&mut [u64; SLOTS]"),
        BodyParamDecl::new("at", "usize"),
    ];
    alias_frame.mutable_indexed_params = vec!["data".to_string()];
    alias_frame.fixed_array_params = vec!["data".to_string()];
    alias_frame.mutable_indexed = vec![MutableIndexedFrame::new("data", array_type())];
    alias_frame.shared_indexed.clear();
    let error =
        body_equivalence_obligation(&function_body(alias_source, "alias"), "", &alias_frame)
            .expect_err("shared indexed actual may not overlap an exclusive actual");
    assert!(
        error
            .to_string()
            .contains("aliases exclusive access path `data` through shared indexed root `data`"),
        "{error}"
    );

    let mut mismatch = mixed_array_frame("mixed_array_pipeline");
    mismatch.shared_indexed = vec![SharedIndexedFrame::new(
        "right",
        Type::Array {
            elem: Box::new(Type::Prim(PrimType::U64)),
            len: ArrayLen::Literal {
                value: 8,
                raw: "8".to_string(),
            },
        },
    )];
    let error = body_equivalence_obligation(
        &function_body(MIXED_ARRAY_SOURCE, "mixed_array_pipeline"),
        "",
        &mismatch,
    )
    .expect_err("shared indexed capacity mismatch must fail closed");
    assert!(
        error.to_string().contains("pointee-type mismatch"),
        "{error}"
    );
}
