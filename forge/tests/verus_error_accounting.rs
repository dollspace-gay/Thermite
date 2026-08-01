//! Issue #111: frontend failures without a structured count stay count-unknown.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_verus_error_count_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn forge(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap()
}

#[test]
fn frontend_rejection_omits_an_unknown_error_count_and_publishes_nothing() {
    let temp = TempDir::new();
    let bundle = temp.0.join("frontend-rejection.verified");
    let output = forge(&[
        "build",
        "conformance/verified-composition/kernel_bytes.th",
        "--level",
        "l3",
        "--compose-export",
        "model_identity",
        "--compose-shell",
        "conformance/verified-composition/frontend_array_rejection.rs",
        "--crate-name",
        "frontend_error_count",
        "--target",
        "kernel",
        "--out",
        bundle.to_string_lossy().as_ref(),
    ]);
    assert!(!output.status.success());
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        diagnostics.contains("Array literals are not supported with --no-vstd"),
        "the reproduction did not reach the expected frontend rejection: {diagnostics}",
    );
    assert!(!diagnostics.contains("18446744073709551615"));
    assert!(
        !diagnostics.contains("failed (errors="),
        "Forge fabricated a numeric count for a count-unknown frontend rejection: {diagnostics}",
    );
    assert!(!bundle.exists(), "frontend rejection published a bundle");
}
