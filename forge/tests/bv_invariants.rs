//! Fixed-width struct and loop invariants, end to end through lowering and checking.

#![cfg(feature = "bv")]

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("forge is in the workspace")
        .to_path_buf()
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn run_check(path: &std::path::Path) -> (Option<i32>, Vec<Value>, String) {
    let output = Command::new(forge_bin())
        .args(["check", "--engine", "bv"])
        .arg(path)
        .arg("--json")
        .output()
        .expect("run forge");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let certificates = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "forge emitted invalid JSON: {error}\nstdout:\n{}\nstderr:\n{stderr}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (output.status.code(), certificates, stderr)
}

fn certificate<'a>(certificates: &'a [Value], item: &str) -> &'a Value {
    certificates
        .iter()
        .find(|certificate| certificate["item"] == item)
        .unwrap_or_else(|| panic!("missing certificate for {item}: {certificates:?}"))
}

#[test]
fn tagged_invariants_lower_and_remain_visible() {
    let fixture = repo_root().join("conformance/forge/bv_invariants.th");
    let source = std::fs::read_to_string(&fixture).expect("read fixture");
    let parsed = thermite_syntax::parse(&source);
    assert!(parsed.is_clean(), "fixture parses: {:?}", parsed.errors);
    let lowered = thermite_lower::lower(&parsed.program).expect("fixture lowers");
    assert!(
        lowered.contains(".wrapping_add(") && lowered.contains(".wrapping_sub("),
        "bare tags use literal wrapping operations:\n{lowered}"
    );
    assert!(
        lowered.contains("((i as u8) + 1u8)"),
        "nowrap keeps the u8 domain and leaves no-overflow to Verus:\n{lowered}"
    );
    assert!(
        lowered.contains("(self.bits as u8)") && lowered.contains("(self.amount as u8)"),
        "struct fields are interpreted at the tag width:\n{lowered}"
    );

    let (code, certificates, stderr) = run_check(&fixture);
    assert_eq!(code, Some(0), "fixture must certify:\n{stderr}");
    let advance = certificate(&certificates, "advance");
    assert_eq!(advance["level"], "L3");
    let shadows: Vec<_> = advance["obligations"]
        .as_array()
        .expect("obligations")
        .iter()
        .filter(|obligation| obligation.get("bv_shadow").is_some())
        .collect();
    assert_eq!(shadows.len(), 2, "both tagged loop invariants are listed");
    assert_eq!(shadows[0]["engine"], "verus");
    assert_eq!(shadows[0]["verdict"]["kind"], "Proved");
    assert!(shadows[1]["bv_shadow"]["nowrap_obligation"]
        .as_str()
        .is_some_and(|text| text.contains("discharged")));

    let word = certificate(&certificates, "Word");
    let struct_shadows: Vec<_> = word["obligations"]
        .as_array()
        .expect("struct obligations")
        .iter()
        .filter(|obligation| obligation.get("bv_shadow").is_some())
        .collect();
    assert_eq!(struct_shadows.len(), 1, "the struct invariant is listed");
}

#[test]
fn a_false_tagged_invariant_is_not_certified() {
    let path = std::env::temp_dir().join(format!(
        "thermite_bad_bv_invariant_{}.th",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "fn bad(start: u64) -> u64\n\
             req true\n\
             ens result == start\n\
             fx pure\n\
         {\n\
             let mut i: u64 = start;\n\
             while i < 1 inv@bv8 i == 0 dec 1 - i { i = i + 1; }\n\
             start\n\
         }\n",
    )
    .expect("write temporary fixture");
    let (code, certificates, stderr) = run_check(&path);
    let _ = std::fs::remove_file(&path);

    assert_ne!(code, Some(0), "a false invariant must fail:\n{stderr}");
    let bad = certificate(&certificates, "bad");
    assert_ne!(bad["level"], "L3");
    let shadow = bad["obligations"]
        .as_array()
        .expect("obligations")
        .iter()
        .find(|obligation| obligation.get("bv_shadow").is_some())
        .expect("failed invariant remains visible");
    assert_eq!(shadow["status"], "failed");
    assert_ne!(shadow["verdict"]["kind"], "Proved");
}
