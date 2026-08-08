//! End-to-end lowering/proof coverage for structural fixed-array relations over
//! finite plain Thermite records. The independent TV teeth live in
//! `thermite-tv`/Forge; this test owns exact generated helper composition and a
//! real Verus discharge.

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_lower::{lower, lower_l1};

const SOURCE: &str = r#"
const WORDS: usize = 2;
const SLOTS: usize = 4;

struct Stamp {
  words: [u64; WORDS],
  flags: (bool, u8),
}

struct Slot {
  stamp: Stamp,
  owner: usize,
}

fn equal(left: [Slot; SLOTS], right: [Slot; SLOTS]) -> bool
req true
ens result == left.array_eq(right)
fx pure
{
  left.array_eq(right)
}

fn same_except(left: [Slot; SLOTS], right: [Slot; SLOTS], changed: usize) -> bool
req true
ens result == left.array_same_except(right, changed)
fx pure
{
  left.array_same_except(right, changed)
}
"#;

fn program() -> thermite_syntax::Program {
    let parsed = thermite_syntax::parse(SOURCE);
    assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("plain aggregate relations must validate");
    parsed.program
}

fn verus_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("VERUS_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    let output = Command::new("which").arg("verus").output().ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn run_verus(path: &Path) -> Option<(bool, String)> {
    let output = Command::new(verus_bin()?)
        .args(["-Z", "no-codegen"])
        .arg(path)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success(), combined))
}

#[test]
fn emits_recursive_structural_helpers_and_l1_equality_only_for_plain_records() {
    let l3 = lower(&program()).expect("aggregate relation L3 lowering");
    assert!(
        l3.contains("fn __thermite_element_eq_Array2OfPrimU64"),
        "nested array equality must receive an exact value bridge:\n{l3}"
    );
    assert!(
        l3.contains("fn __thermite_element_eq_Tuple2OfPrimBoolOfPrimU8"),
        "tuple fields must compose structurally:\n{l3}"
    );
    assert!(
        l3.contains("fn __thermite_element_eq_Struct_Stamp"),
        "nested records must receive their own exact comparator:\n{l3}"
    );
    assert!(
        l3.contains("fn __thermite_element_eq_Struct_Slot"),
        "outer records must compose the nested comparator:\n{l3}"
    );
    assert!(
        l3.contains("impl<const N: usize> __thermite_FixedArrayEq for [Slot; N]"),
        "record arrays must use the same exact scan contract as scalar arrays:\n{l3}"
    );
    assert!(l3.contains("assert(*left == *right);"), "{l3}");

    let l1 = lower_l1(&program()).expect("aggregate relation L1 lowering");
    assert!(
        l1.contains("#[derive(Clone, PartialEq, Eq)]\n#[allow(dead_code)]\nstruct Stamp"),
        "relation-reachable nested records need runnable structural equality:\n{l1}"
    );
    assert!(
        l1.contains("#[derive(Clone, PartialEq, Eq)]\n#[allow(dead_code)]\nstruct Slot"),
        "relation element records need runnable structural equality:\n{l1}"
    );
}

#[test]
fn nested_plain_record_relations_verify_with_real_verus() {
    let source = lower(&program()).expect("aggregate relation L3 lowering");
    let path = std::env::temp_dir().join("thermite_aggregate_array_relations.rs");
    std::fs::write(&path, &source).expect("write aggregate-relation Verus unit");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "aggregate relation library did not verify:\n{output}\n--- emitted ---\n{source}"
        ),
        None => eprintln!("SKIP: verus unavailable; structural emission assertions still ran"),
    }
}

#[test]
fn dropped_record_field_is_rejected_by_the_direct_verus_contract() {
    let source = lower(&program()).expect("aggregate relation L3 lowering");
    let needle = "(left.owner) == (right.owner)";
    assert!(
        source.contains(needle),
        "owner comparison missing:\n{source}"
    );
    let mutant = source.replacen(needle, "true", 1);
    let path = std::env::temp_dir().join("thermite_aggregate_array_dropped_field.rs");
    std::fs::write(&path, &mutant).expect("write dropped-field Verus mutant");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            !success && !output.contains("verified, 0 errors"),
            "dropping one record field must violate the exact element comparator contract:\n\
             {output}\n--- mutant ---\n{mutant}"
        ),
        None => eprintln!("SKIP: verus unavailable; dropped-field source mutation still formed"),
    }
}

#[test]
fn aggregate_relations_remain_runnable_at_l1() {
    let source = lower_l1(&program()).expect("aggregate relation L1 lowering");
    let path = std::env::temp_dir().join("thermite_aggregate_array_relations_l1.rs");
    std::fs::write(&path, &source).expect("write aggregate-relation L1 unit");
    let output = Command::new("rustc")
        .args(["--crate-type", "lib"])
        .arg(&path)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn rustc for aggregate-relation L1 unit");
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    assert!(
        output.status.success(),
        "aggregate relation L1 output did not compile:\n{combined}\n--- emitted ---\n{source}"
    );
}
