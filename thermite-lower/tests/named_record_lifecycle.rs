use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_lower::{lower, lower_l1};

const SOURCE: &str = r#"
#[opaque] struct State {
  generation: u64,
  occupied: bool,
}

fn state_new(generation: u64, occupied: bool) -> State
  req true
  ens result.generation == generation
  ens result.occupied == occupied
  fx pure
{
  State { generation: generation, occupied: occupied }
}

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

fn program() -> thermite_syntax::Program {
    let parsed = thermite_syntax::parse(SOURCE);
    assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("named-record lifecycle must validate");
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
fn l3_and_l1_emit_the_exact_direct_field_transition() {
    let l3 = lower(&program()).expect("named-record L3 lowering");
    assert!(l3.contains("state.generation = next;"), "{l3}");
    assert!(l3.contains("let previous: bool = state.occupied;"), "{l3}");
    assert!(l3.contains("final(state).generation == next"), "{l3}");
    assert!(
        l3.contains("final(state).occupied == old(state).occupied"),
        "{l3}"
    );

    let l1 = lower_l1(&program()).expect("named-record L1 lowering");
    assert!(l1.contains("state.generation = next;"), "{l1}");
    assert!(l1.contains("let previous: bool = state.occupied;"), "{l1}");
}

#[test]
fn direct_named_record_lifecycle_verifies_with_real_verus() {
    let source = lower(&program()).expect("named-record L3 lowering");
    let path = std::env::temp_dir().join("thermite_named_record_lifecycle.rs");
    std::fs::write(&path, &source).expect("write named-record Verus unit");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "named-record lifecycle did not verify:\n{output}\n--- emitted ---\n{source}"
        ),
        None => eprintln!("SKIP: verus unavailable; structural emission assertions still ran"),
    }
}
