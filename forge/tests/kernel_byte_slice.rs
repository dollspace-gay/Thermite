//! Issue #108: exact-source byte-slice proofs in a `--no-vstd` kernel build.

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

fn forge(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_kernel_bytes_{}_{}",
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

fn build_args(bundle: &Path, shell: &str, crate_name: &str) -> Vec<String> {
    vec![
        "build".to_string(),
        "conformance/verified-composition/kernel_bytes.th".to_string(),
        "--level".to_string(),
        "l3".to_string(),
        "--compose-export".to_string(),
        "model_identity".to_string(),
        "--compose-shell".to_string(),
        shell.to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "--target".to_string(),
        "kernel".to_string(),
        "--out".to_string(),
        bundle.to_string_lossy().to_string(),
    ]
}

fn refs(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}

fn codegen_rustc(bundle: &Path) -> Command {
    let evidence: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/toolchain.json")).unwrap()).unwrap();
    let toolchain = evidence["artifact_codegen"]["rustup_toolchain"]
        .as_str()
        .unwrap();
    let mut command = Command::new("rustup");
    command.args(["run", toolchain, "rustc"]);
    command
}

#[test]
fn kernel_byte_slice_is_verified_executable_freestanding_and_reproducible() {
    let temp = TempDir::new();
    let first = temp.0.join("first.verified");
    let second = temp.0.join("second.verified");
    let first_args = build_args(
        &first,
        "conformance/verified-composition/kernel_bytes_shell.rs",
        "thermite_kernel_bytes",
    );
    let second_args = build_args(
        &second,
        "conformance/verified-composition/kernel_bytes_shell.rs",
        "thermite_kernel_bytes",
    );
    assert_success(&forge(&refs(&first_args)));
    assert_success(&forge(&[
        "verify-build",
        first.to_string_lossy().as_ref(),
        "--replay",
    ]));

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(first.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    let args = plan["expected_verus_args"].as_array().unwrap();
    for expected in [
        "--no-vstd",
        "--no-cheating",
        "vstd=<KERNEL_VSTD_VIR>",
        "vstd=<KERNEL_VSTD_RLIB>",
    ] {
        assert!(args.iter().any(|arg| arg == expected));
    }

    let toolchain: serde_json::Value =
        serde_json::from_slice(&fs::read(first.join("evidence/toolchain.json")).unwrap()).unwrap();
    let model = &toolchain["kernel_vstd_model"];
    for digest in [
        "vir_sha256",
        "source_sha256",
        "link_source_sha256",
        "link_rlib_sha256",
    ] {
        assert_eq!(model[digest].as_str().unwrap().len(), 64);
    }
    assert_eq!(model["link_source_name"], "kernel-vstd-link.rs");
    assert!(model["source_file_count"].as_u64().unwrap() > 0);
    assert_eq!(
        fs::read(first.join("evidence/kernel-vstd-link.rs")).unwrap(),
        fs::read(root().join("forge/src/kernel_vstd_link.rs")).unwrap(),
    );

    let source = fs::read_to_string(first.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n#![crate_type = \"rlib\"]"));
    assert!(source.contains("use vstd::prelude::*;"));
    assert!(source.contains("pub fn read_u32_le(bytes: &[u8]"));
    assert!(source.contains("pub fn read_u64_le(bytes: &[u8]"));
    assert!(source.contains("result == spec_read_u32_le(bytes, offset as int)"));
    assert!(source.contains("result == spec_read_u64_le(bytes, offset as int)"));
    for forbidden in ["external_body", "assume(", "admit(", "decreases *"] {
        assert!(!source.contains(forbidden));
    }

    let artifact = first.join("artifact/libthermite_kernel_bytes.rlib");
    let deps = first.join("artifact/deps");
    let host_consumer = temp.0.join("host-consumer");
    let host = codegen_rustc(&first)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-composition/kernel_bytes_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!("thermite_kernel_bytes={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(&host_consumer)
        .output()
        .unwrap();
    assert_success(&host);
    assert_success(&Command::new(&host_consumer).output().unwrap());

    let low_gate = codegen_rustc(&first)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-composition/kernel_bytes_freestanding.rs",
            "--crate-type=rlib",
        ])
        .arg("--extern")
        .arg(format!("thermite_kernel_bytes={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(temp.0.join("libfreestanding_gate.rlib"))
        .output()
        .unwrap();
    assert_success(&low_gate);

    let high_gate = codegen_rustc(&first)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-composition/kernel_bytes_freestanding.rs",
        ])
        .arg("--extern")
        .arg(format!("thermite_kernel_bytes={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort", "-C", "link-arg=-nostartfiles"])
        .arg("-o")
        .arg(temp.0.join("freestanding-gate"))
        .output()
        .unwrap();
    assert_success(&high_gate);

    assert_success(&forge(&refs(&second_args)));
    for relative in [
        "receipt.json",
        "evidence/source.verus.rs",
        "artifact/libthermite_kernel_bytes.rlib",
        "artifact/deps/libvstd.rlib",
    ] {
        assert_eq!(
            fs::read(first.join(relative)).unwrap(),
            fs::read(second.join(relative)).unwrap(),
            "non-reproducible `{relative}`",
        );
    }
}

#[test]
fn wrong_content_and_out_of_bounds_shells_are_rejected() {
    let temp = TempDir::new();
    for (index, (shell, expected_failure)) in [
        (
            "conformance/verified-composition/kernel_bytes_wrong_content.rs",
            "postcondition not satisfied",
        ),
        (
            "conformance/verified-composition/kernel_bytes_oob.rs",
            "precondition not satisfied",
        ),
    ]
    .iter()
    .enumerate()
    {
        let bundle = temp.0.join(format!("negative-{index}.verified"));
        let args = build_args(&bundle, shell, &format!("kernel_bytes_negative_{index}"));
        let output = forge(&refs(&args));
        assert!(
            !output.status.success(),
            "negative shell `{shell}` unexpectedly built: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let diagnostics = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(
            diagnostics.contains(expected_failure),
            "negative shell `{shell}` failed for the wrong reason: {diagnostics}",
        );
        assert!(!bundle.exists(), "negative shell published a bundle");
    }
}
