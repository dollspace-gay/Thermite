use std::path::PathBuf;
use std::process::Command;

use thermite_syntax::Item;
use thermite_tv::obligation::{
    body_equivalence_obligation, exec_equivalence_obligation, BodyObligationFrame, BodyParamDecl,
    ExecObligationFrame, ExecParamDecl,
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
            output.contains("postcondition not satisfied"),
            "negative fixed-array TV must fail at the extensional result postcondition:\n{output}"
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
