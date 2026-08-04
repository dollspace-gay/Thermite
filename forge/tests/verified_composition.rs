//! Exact-source rich-state Thermite/direct-Verus composition acceptance.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

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

fn forge_with_fault(args: &[&str], fault: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(args)
        .env("THERMITE_L3_TEST_FAULT", fault)
        .current_dir(root())
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_verified_composition_{name}_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
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

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let destination = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn build_args(bundle: &Path) -> Vec<String> {
    vec![
        "build".to_string(),
        "conformance/verified-composition/probe.th".to_string(),
        "--level".to_string(),
        "l3".to_string(),
        "--compose-export".to_string(),
        "probe_step".to_string(),
        "--compose-shell".to_string(),
        "conformance/verified-composition/probe_shell.rs".to_string(),
        "--crate-name".to_string(),
        "thermite_probe".to_string(),
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

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn frozen_registry_directly_refines_the_exact_reachable_boundary() {
    let temp = TempDir::new("frozen-registry");
    let bundle = temp.0.join("primitive.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    let args = [
        "build",
        "conformance/verified-composition/frozen_primitive.th",
        "--level",
        "l3",
        "--compose-export",
        "primitive_observation",
        "--compose-shell",
        "conformance/verified-composition/frozen_primitive_shell.rs",
        "--primitive-registry",
        "conformance/verified-composition/frozen_primitive_registry.json",
        "--crate-name",
        "thermite_frozen_primitive",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
    ];
    assert_success(&forge(&args));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains("fn platform_identity(value: u64) -> (result: u64)"));
    assert!(source.contains("frozen_primitive_shell::identity_impl(value)"));
    assert!(!source.contains("external_body"));
    assert!(!source.contains("unimplemented!"));

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    let registry = &plan["composition"]["primitive_registry"];
    assert_eq!(registry["schema"], "thermite.frozen-primitive-registry.v1");
    assert_eq!(registry["entries"][0]["thermite_name"], "platform_identity");
    assert_eq!(registry["entries"][0]["reachable"], true);
    assert_eq!(
        registry["entries"][0]["refinement"],
        "same_crate_verus_checked_wrapper"
    );

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(
        receipt["binding"]["composition"]["reachable_primitive_count"],
        1
    );
    assert_eq!(
        receipt["binding"]["composition"]["discharged_refinement_obligations"],
        3
    );
    let members = receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap();
    assert!(members.iter().any(|member| {
        member["name"] == "platform_identity"
            && member["kind"] == "frozen_primitive_boundary"
            && member["achieved"] == "L3-direct-refinement"
    }));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    assert!(tv["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["verdict"] == "faithful"));
    assert!(tv["rows"].as_array().unwrap().iter().any(|row| {
        row["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("frozen primitive completion"))
    }));

    let tampered = temp.0.join("tampered.verified");
    copy_tree(&bundle, &tampered);
    let registry_path = tampered.join("evidence/frozen-primitive-registry.json");
    let mut bytes = fs::read(&registry_path).unwrap();
    bytes.push(b' ');
    fs::write(&registry_path, bytes).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success()
    );

    let fault_bundle = temp.0.join("registry-race-must-not-publish.verified");
    let fault_args = vec![
        "build".to_string(),
        "conformance/verified-composition/frozen_primitive.th".to_string(),
        "--level".to_string(),
        "l3".to_string(),
        "--compose-export".to_string(),
        "primitive_observation".to_string(),
        "--compose-shell".to_string(),
        "conformance/verified-composition/frozen_primitive_shell.rs".to_string(),
        "--primitive-registry".to_string(),
        "conformance/verified-composition/frozen_primitive_registry.json".to_string(),
        "--crate-name".to_string(),
        "thermite_frozen_primitive_fault".to_string(),
        "--target".to_string(),
        "kernel".to_string(),
        "--out".to_string(),
        fault_bundle.to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let fault = forge_with_fault(
        &fault_args.iter().map(String::as_str).collect::<Vec<_>>(),
        "composition-after-plan-registry-mutation",
    );
    assert_eq!(fault.status.code(), Some(1));
    assert!(!fault_bundle.exists());
    assert!(String::from_utf8_lossy(&fault.stdout).contains("binding"));

    let drift_dir = temp.0.join("drift");
    fs::create_dir(&drift_dir).unwrap();
    let bad_shell = b"pub fn identity_impl(value: u64) -> (result: u64) ensures result == value, { 0 }\npub fn observe_bound_primitive() -> (result: u64) ensures result == 7, { primitive_observation(7) }\n";
    let bad_shell_path = drift_dir.join("frozen_primitive_shell.rs");
    fs::write(&bad_shell_path, bad_shell).unwrap();
    let mut bad_registry: serde_json::Value = serde_json::from_slice(
        &fs::read(root().join("conformance/verified-composition/frozen_primitive_registry.json"))
            .unwrap(),
    )
    .unwrap();
    bad_registry["entries"][0]["implementation"]["source_sha256"] =
        serde_json::json!(digest(bad_shell));
    let bad_registry_path = drift_dir.join("registry.json");
    fs::write(
        &bad_registry_path,
        serde_json::to_vec_pretty(&bad_registry).unwrap(),
    )
    .unwrap();
    let rejected_bundle = temp.0.join("must-not-publish.verified");
    let rejected = forge(&[
        "build",
        "conformance/verified-composition/frozen_primitive.th",
        "--level",
        "l3",
        "--compose-export",
        "primitive_observation",
        "--compose-shell",
        bad_shell_path.to_string_lossy().as_ref(),
        "--primitive-registry",
        bad_registry_path.to_string_lossy().as_ref(),
        "--crate-name",
        "thermite_frozen_primitive_bad",
        "--target",
        "kernel",
        "--out",
        rejected_bundle.to_string_lossy().as_ref(),
        "--json",
    ]);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(!rejected_bundle.exists());
    assert!(String::from_utf8_lossy(&rejected.stdout).contains("whole-crate-verus"));
}

#[test]
fn probe_state_composition_is_exact_private_linkable_and_reproducible() {
    let temp = TempDir::new("probe");
    let first = temp.0.join("first.verified");
    let second = temp.0.join("second.verified");
    let third = temp.0.join("third.verified");
    let first_args = build_args(&first);
    let second_args = build_args(&second);
    let third_args = build_args(&third);
    assert_success(&forge(&refs(&first_args)));
    assert_success(&forge(&[
        "verify-build",
        first.to_string_lossy().as_ref(),
        "--replay",
    ]));

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(first.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(
        receipt["schema"],
        "thermite.verified-composition-receipt.v1"
    );
    assert_eq!(
        receipt["binding"]["schema"],
        "thermite.verified-composition-receipt.v1"
    );
    for digest in [
        "lowered_thermite_sha256",
        "direct_verus_set_sha256",
        "inventory_sha256",
        "combined_source_sha256",
    ] {
        assert_eq!(
            receipt["binding"]["composition"][digest]
                .as_str()
                .unwrap()
                .len(),
            64
        );
    }

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(first.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["schema"], "thermite.combined-artifact-plan.v1");
    assert_eq!(
        plan["composition"]["composition_exports"][0]["visibility"],
        "crate"
    );
    assert_eq!(
        plan["composition"]["composition_exports"][0]["return_type"],
        "(ProbeState,ProbeAction)"
    );
    let types = plan["composition"]["composition_exports"][0]["type_closure"]
        .as_array()
        .unwrap();
    assert!(types.iter().any(|value| value == "Vec<u64>"));
    assert!(types.iter().any(|value| {
        value
            .as_str()
            .is_some_and(|value| value.starts_with("struct ProbeState"))
    }));
    assert!(plan["expected_verus_args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "--no-vstd"));

    let source = fs::read_to_string(first.join("evidence/source.verus.rs")).unwrap();
    assert_eq!(source.matches("verus!").count(), 1);
    assert!(source.contains("pub(crate) fn probe_step"));
    assert!(!source.contains("pub fn probe_step"));
    assert!(source.contains("macro_rules! __thermite_deterministic_enum"));
    assert!(source.contains("#[verus::internal(verus_macro)]"));
    assert!(source.contains("Store { owner: u64, generation: u64, slot: u64, value: u64 }"));
    assert!(source.contains("pub mod probe_shell"));
    assert!(source.contains("pub fn boot_observation"));
    for forbidden in ["external_body", "assume(", "admit(", "decreases *"] {
        assert!(!source.contains(forbidden));
    }
    let lowered = fs::read_to_string(first.join("evidence/lowered-thermite.verus.rs")).unwrap();
    assert!(source.starts_with(lowered.strip_suffix("}\n").unwrap()));
    assert_eq!(
        fs::read(first.join("evidence/direct-verus/00-probe_shell.rs")).unwrap(),
        fs::read(root().join("conformance/verified-composition/probe_shell.rs")).unwrap()
    );

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(first.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    assert!(tv["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["verdict"] == "faithful"));

    let artifact = first.join("artifact/libthermite_probe.rlib");
    let artifact_bytes = fs::read(&artifact).unwrap();
    for randomized_helper in [
        "arrow_owner",
        "arrow_generation",
        "arrow_slot",
        "arrow_value",
    ] {
        assert!(!artifact_bytes
            .windows(randomized_helper.len())
            .any(|window| window == randomized_helper.as_bytes()));
    }
    let deps = first.join("artifact/deps");
    let host_consumer = temp.0.join("host-consumer");
    let host = codegen_rustc(&first)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-composition/probe_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!("thermite_probe={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(&host_consumer)
        .output()
        .unwrap();
    assert_success(&host);
    assert_success(&Command::new(&host_consumer).output().unwrap());

    let private = codegen_rustc(&first)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-composition/private_step_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!("thermite_probe={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(temp.0.join("must-not-link"))
        .output()
        .unwrap();
    assert!(!private.status.success());
    assert!(String::from_utf8_lossy(&private.stderr).contains("private"));

    assert_success(&forge(&refs(&second_args)));
    assert_success(&forge(&refs(&third_args)));
    assert_eq!(
        fs::read(first.join("receipt.json")).unwrap(),
        fs::read(second.join("receipt.json")).unwrap()
    );
    assert_eq!(
        fs::read(first.join("receipt.json")).unwrap(),
        fs::read(third.join("receipt.json")).unwrap()
    );
    assert_eq!(
        artifact_bytes,
        fs::read(second.join("artifact/libthermite_probe.rlib")).unwrap()
    );
    assert_eq!(
        fs::read(&artifact).unwrap(),
        fs::read(third.join("artifact/libthermite_probe.rlib")).unwrap()
    );

    let tampered = temp.0.join("tampered.verified");
    copy_tree(&first, &tampered);
    let shell = tampered.join("evidence/direct-verus/00-probe_shell.rs");
    let mut bytes = fs::read(&shell).unwrap();
    bytes.push(b' ');
    fs::write(shell, bytes).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success()
    );
}

#[test]
fn composition_faults_and_nonpass_evidence_publish_nothing() {
    let temp = TempDir::new("faults");
    for (index, fault) in [
        "composition-after-plan-shell-mutation",
        "certificate-l2",
        "tv-contract-divergent",
    ]
    .iter()
    .enumerate()
    {
        let bundle = temp.0.join(format!("fault-{index}.verified"));
        let args = build_args(&bundle);
        let output = forge_with_fault(&refs(&args), fault);
        assert!(
            !output.status.success(),
            "fault `{fault}` unexpectedly built: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!bundle.exists(), "fault `{fault}` published a bundle");
    }
}
