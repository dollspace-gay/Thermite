//! Conformance pins for the reusable `#[opaque]` library-state primitive.
//! The package checker owns cross-Thermite-module construction; these tests own
//! the independent L3/Rust representation barrier and its usable abstract spec
//! surface.

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_lower::{lower_l3_library, L3Export, L3ExportVisibility, L3LibraryTarget};

const SOURCE: &str = r#"
#[opaque] struct State {
  value: u64,
} inv value <= u64::MAX

struct WrappedState {
  state: State,
}

struct Plain {
  value: u64,
}

spec fn state_value(state: &State) -> u64
  dec state.value
{
  state.value
}

spec fn wrapped_value(state: &WrappedState) -> u64
  dec state.state.value
{
  state.state.value
}

spec fn scalar_identity(value: u64) -> u64
  dec value
{
  value
}

fn state_new(value: u64) -> State
  req true
  ens state_value(&result) == value
  fx pure
{
  State { value: value }
}

fn state_read(state: &State) -> u64
  req true
  ens result == state_value(state)
  fx pure
{
  state.value
}
"#;

fn emitted() -> String {
    let parsed = thermite_syntax::parse(SOURCE);
    assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
    let exports = [
        L3Export {
            source_name: "state_new".to_string(),
            public_name: "state_new".to_string(),
            wrapped: false,
            visibility: L3ExportVisibility::Public,
        },
        L3Export {
            source_name: "state_read".to_string(),
            public_name: "state_read".to_string(),
            wrapped: false,
            visibility: L3ExportVisibility::Public,
        },
    ];
    lower_l3_library(&parsed.program, &exports, L3LibraryTarget::Std)
        .expect("opaque state must lower as an L3 library")
}

fn verus_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("VERUS_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    if let Ok(output) = Command::new("which").arg("verus").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(home).join(".local/bin/verus");
        if candidate.exists() {
            return Some(candidate);
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
fn opaque_fields_are_not_external_constructors_and_specs_are_abstract() {
    let source = emitted();
    assert!(
        source.contains("pub struct State {\n    pub(crate) value: u64,"),
        "an external Rust crate must not receive a public State field:\n{source}"
    );
    assert!(
        source.contains("pub closed spec fn state_value("),
        "the opaque observer must remain public but abstract externally:\n{source}"
    );
    assert!(
        source.contains("pub closed spec fn well_formed(&self) -> bool"),
        "an opaque representation invariant must also remain abstract externally:\n{source}"
    );
    assert!(
        source.contains("pub closed spec fn wrapped_value("),
        "opacity must propagate through a named wrapper type:\n{source}"
    );
    assert!(
        source.contains("pub open spec fn scalar_identity("),
        "unrelated scalar specs must retain the ordinary open surface:\n{source}"
    );
    assert!(
        source.contains("pub struct Plain {\n    pub value: u64,"),
        "plain structs must retain their existing public representation:\n{source}"
    );
    assert!(source.contains("pub fn state_new"));
    assert!(source.contains("pub fn state_read"));
}

#[test]
fn exported_opaque_constructor_and_observer_verify_in_the_defining_module() {
    let source = emitted();
    let path = std::env::temp_dir().join("thermite_opaque_state.rs");
    std::fs::write(&path, &source).expect("write opaque-state Verus unit");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "opaque-state library did not verify:\n{output}\n--- emitted ---\n{source}"
        ),
        None => eprintln!("SKIP: verus unavailable; opaque-state structural assertions still ran"),
    }
}
