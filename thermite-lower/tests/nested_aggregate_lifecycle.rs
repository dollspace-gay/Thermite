use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_lower::{lower, lower_l1};

const SOURCE: &str = r#"
const SLOTS: usize = 2;
struct Inner { value: u64, guard: u64 }
struct Nested { inner: Inner, slots: [u64; SLOTS], tag: u64 }

fn nested_owned(state: Nested, index: usize, next: u64) -> Nested
  req index < SLOTS && next < 1000
  ens result.inner.value == next
  ens result.inner.guard == state.inner.guard
  ens result.slots[index] == next + 1
  ens result.tag == state.tag
  fx pure
{
  let mut updated: Nested = state;
  updated.inner.value = next;
  updated.slots[index] = updated.inner.value + 1;
  updated
}

fn nested_borrowed(state: &mut Nested, index: usize, next: u64) -> u64
  req index < SLOTS && next < 1000
  ens result == next
  ens final(state).inner.value == next
  ens final(state).inner.guard == old(state).inner.guard
  ens final(state).slots[index] == next + 1
  ens final(state).tag == old(state).tag
  fx pure
{
  state.inner.value = next;
  state.slots[index] = state.inner.value + 1;
  state.inner.value
}
"#;

fn program() -> thermite_syntax::Program {
    let parsed = thermite_syntax::parse(SOURCE);
    assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("nested aggregate lifecycle must validate");
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
fn both_assurance_lowerers_emit_the_exact_nested_lvalues() {
    for emitted in [
        lower(&program()).expect("nested aggregate L3 lowering"),
        lower_l1(&program()).expect("nested aggregate L1 lowering"),
    ] {
        assert!(emitted.contains("updated.inner.value = next;"), "{emitted}");
        assert!(
            emitted.contains("updated.slots[index] = updated.inner.value + 1;"),
            "{emitted}"
        );
        assert!(emitted.contains("state.inner.value = next;"), "{emitted}");
        assert!(
            emitted.contains("state.slots[index] = state.inner.value + 1;"),
            "{emitted}"
        );
    }
}

#[test]
fn generated_nested_aggregate_lifecycle_verifies_with_real_verus() {
    let source = lower(&program()).expect("nested aggregate L3 lowering");
    let path = std::env::temp_dir().join("thermite_nested_aggregate_lifecycle.rs");
    std::fs::write(&path, &source).expect("write nested aggregate Verus unit");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "nested aggregate lifecycle did not verify:\n{output}\n--- emitted ---\n{source}"
        ),
        None => eprintln!("SKIP: verus unavailable; structural emission assertions still ran"),
    }
}
