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
    assert_eq!(
        registry["target"]["target_features"],
        serde_json::json!(["sse2"])
    );
    assert_eq!(registry["entries"][0]["thermite_name"], "platform_identity");
    assert_eq!(registry["entries"][0]["reachable"], true);
    assert_eq!(
        registry["entries"][0]["refinement"],
        "same_crate_verus_checked_wrapper"
    );
    let expected_args = plan["expected_verus_args"].as_array().unwrap();
    assert!(expected_args
        .windows(2)
        .any(|pair| { pair[0] == "-C" && pair[1] == "target-feature=+sse2" }));
    let verus_result: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/verus-result.json")).unwrap())
            .unwrap();
    assert_eq!(verus_result["args"], plan["expected_verus_args"]);

    let unsupported_registry_path = temp.0.join("unsupported-feature.registry.json");
    let mut unsupported_registry: serde_json::Value = serde_json::from_slice(
        &fs::read(root().join("conformance/verified-composition/frozen_primitive_registry.json"))
            .unwrap(),
    )
    .unwrap();
    unsupported_registry["target"]["target_features"] =
        serde_json::json!(["imaginary-thermite-feature"]);
    fs::write(
        &unsupported_registry_path,
        serde_json::to_vec_pretty(&unsupported_registry).unwrap(),
    )
    .unwrap();
    let unsupported_bundle = temp.0.join("unsupported-feature-must-not-publish.verified");
    let unsupported = forge(&[
        "build",
        "conformance/verified-composition/frozen_primitive.th",
        "--level",
        "l3",
        "--compose-export",
        "primitive_observation",
        "--compose-shell",
        "conformance/verified-composition/frozen_primitive_shell.rs",
        "--primitive-registry",
        unsupported_registry_path.to_string_lossy().as_ref(),
        "--crate-name",
        "thermite_frozen_primitive_unsupported_feature",
        "--target",
        "kernel",
        "--out",
        unsupported_bundle.to_string_lossy().as_ref(),
        "--json",
    ]);
    assert_eq!(unsupported.status.code(), Some(1));
    assert!(!unsupported_bundle.exists());
    assert!(String::from_utf8_lossy(&unsupported.stdout)
        .contains("not in the pinned codegen rustc target-feature inventory"));

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
fn separate_primitive_source_interface_rlib_and_object_are_exact_and_replayed() {
    let temp = TempDir::new("separate-primitive");
    let bundle = temp.0.join("separate.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    let args = [
        "build",
        "conformance/verified-composition/frozen_primitive.th",
        "--level",
        "l3",
        "--compose-export",
        "primitive_observation",
        "--compose-shell",
        "conformance/verified-composition/separate_primitive_impl.rs",
        "--compose-shell",
        "conformance/verified-composition/separate_primitive_shell.rs",
        "--primitive-registry",
        "conformance/verified-composition/separate_primitive_registry.json",
        "--crate-name",
        "thermite_separate_primitive",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
    ];
    assert_success(&forge(&args));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains("extern crate separate_primitive_impl;"));
    assert!(source.contains("separate_primitive_impl::identity_impl(value)"));
    assert!(!source.contains("pub fn identity_impl"));

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(
        plan["composition"]["primitive_registry"]["schema"],
        "thermite.frozen-primitive-registry.v2"
    );
    assert_eq!(
        plan["composition"]["primitive_registry"]["entries"][0]["implementation_linkage"],
        "separate_verus_crate"
    );
    let primitive_crates = plan["composition"]["primitive_crates"].as_array().unwrap();
    assert_eq!(primitive_crates.len(), 1);
    assert_eq!(primitive_crates[0]["name"], "separate_primitive_impl");

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    let bound = &receipt["binding"]["composition"]["primitive_crates"][0];
    assert_eq!(bound["name"], "separate_primitive_impl");
    assert_eq!(bound["vir_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(bound["rlib_sha256"].as_str().unwrap().len(), 64);
    assert!(!bound["object_members"].as_array().unwrap().is_empty());
    assert!(bound["object_members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|object| object["name"].as_str().unwrap().ends_with(".o")
            && object["sha256"].as_str().unwrap().len() == 64));

    let consumer = temp.0.join("separate-consumer");
    let linked = codegen_rustc(&bundle)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-composition/separate_primitive_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!(
            "thermite_separate_primitive={}",
            bundle
                .join("artifact/libthermite_separate_primitive.rlib")
                .display()
        ))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            bundle.join("artifact/deps").display()
        ))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(&consumer)
        .output()
        .unwrap();
    assert_success(&linked);
    assert_success(&Command::new(&consumer).output().unwrap());

    for (name, relative) in [
        ("rlib", "artifact/deps/libseparate_primitive_impl.rlib"),
        (
            "vir",
            "evidence/primitive-crates/00-separate_primitive_impl/interface.vir",
        ),
        (
            "authored-source",
            "evidence/primitive-crates/00-separate_primitive_impl/authored.rs",
        ),
    ] {
        let tampered = temp.0.join(format!("tampered-{name}.verified"));
        copy_tree(&bundle, &tampered);
        let path = tampered.join(relative);
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b' ');
        fs::write(path, bytes).unwrap();
        assert!(
            !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
                .status
                .success(),
            "tampered separate primitive {name} was accepted"
        );
    }

    let lying_dir = temp.0.join("lying-source");
    fs::create_dir(&lying_dir).unwrap();
    let lying_source =
        b"pub fn identity_impl(value: u64) -> (result: u64) ensures result == value, { 0 }\n";
    let lying_source_path = lying_dir.join("separate_primitive_impl.rs");
    fs::write(&lying_source_path, lying_source).unwrap();
    let mut lying_registry: serde_json::Value = serde_json::from_slice(
        &fs::read(root().join("conformance/verified-composition/separate_primitive_registry.json"))
            .unwrap(),
    )
    .unwrap();
    lying_registry["entries"][0]["implementation"]["source_sha256"] =
        serde_json::json!(digest(lying_source));
    let lying_registry_path = lying_dir.join("registry.json");
    fs::write(
        &lying_registry_path,
        serde_json::to_vec_pretty(&lying_registry).unwrap(),
    )
    .unwrap();
    let lying_bundle = temp.0.join("lying-must-not-publish.verified");
    let lying = forge(&[
        "build",
        "conformance/verified-composition/frozen_primitive.th",
        "--level",
        "l3",
        "--compose-export",
        "primitive_observation",
        "--compose-shell",
        lying_source_path.to_string_lossy().as_ref(),
        "--compose-shell",
        "conformance/verified-composition/separate_primitive_shell.rs",
        "--primitive-registry",
        lying_registry_path.to_string_lossy().as_ref(),
        "--crate-name",
        "thermite_lying_separate_primitive",
        "--target",
        "kernel",
        "--out",
        lying_bundle.to_string_lossy().as_ref(),
        "--json",
    ]);
    assert_eq!(lying.status.code(), Some(1));
    assert!(!lying_bundle.exists());
    assert!(String::from_utf8_lossy(&lying.stdout).contains("primitive-crate-verus"));
}

#[test]
fn machine_atomic_registry_keeps_the_hardware_residual_visible_and_replays_exactly() {
    let temp = TempDir::new("machine-atomic");
    let bundle = temp.0.join("machine-atomic.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    let args = [
        "build",
        "conformance/verified-composition/machine_atomic.th",
        "--level",
        "l3",
        "--compose-export",
        "machine_atomic_observation",
        "--compose-shell",
        "conformance/verified-composition/machine_atomic_impl.rs",
        "--compose-shell",
        "conformance/verified-composition/machine_atomic_shell.rs",
        "--primitive-registry",
        "conformance/verified-composition/machine_atomic_registry.json",
        "--crate-name",
        "thermite_machine_atomic",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
    ];
    assert_success(&forge(&args));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay"]));

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    let registry = &plan["composition"]["primitive_registry"];
    assert_eq!(registry["schema"], "thermite.frozen-primitive-registry.v3");
    assert_eq!(
        registry["entries"][0]["implementation_linkage"],
        "separate_verus_machine_crate"
    );
    assert_eq!(
        registry["entries"][0]["machine_operation"],
        "p_atomic_u64_seq_cst_roundtrip"
    );
    assert_eq!(
        plan["composition"]["primitive_crates"][0]["proof_basis"],
        "pinned_vstd_machine_model"
    );

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L1");
    assert_eq!(receipt["binding"]["scope"], "to_machine_boundary");
    let gates = receipt["binding"]["strict_gates"].as_array().unwrap();
    assert!(gates.contains(&serde_json::json!("explicit-residual-machine-assumptions")));
    assert!(!gates.contains(&serde_json::json!("complete-end-to-end-closure")));
    assert_eq!(
        receipt["binding"]["composition"]["discharged_refinement_obligations"],
        10
    );
    assert_eq!(
        receipt["binding"]["composition"]["residual_machine_assumptions"],
        3
    );
    let members = receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap();
    assert!(members.iter().any(|member| {
        member["name"] == "machine_atomic_observation"
            && member["kind"] == "executable"
            && member["achieved"] == "L3"
    }));
    assert!(members.iter().any(|member| {
        member["name"] == "machine_atomic_roundtrip"
            && member["kind"] == "frozen_machine_boundary"
            && member["achieved"] == "L1-residual-machine-assumption"
    }));
    assert!(members.iter().any(|member| {
        member["name"] == "machine_atomic_roundtrip::checked_wrapper"
            && member["kind"] == "machine_refinement_wrapper"
            && member["achieved"] == "L3-relative-to-pinned-machine-model"
    }));

    let toolchain: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/toolchain.json")).unwrap()).unwrap();
    let model = &toolchain["kernel_vstd_model"];
    assert_eq!(
        digest(&fs::read(bundle.join("evidence/machine-models/pinned-vstd-atomic.rs")).unwrap()),
        model["atomic_source_sha256"].as_str().unwrap()
    );
    assert_eq!(
        digest(&fs::read(bundle.join("artifact/deps/libvstd.rlib")).unwrap()),
        model["full_rlib_sha256"].as_str().unwrap()
    );
    let generated_primitive = fs::read_to_string(
        bundle.join("evidence/primitive-crates/00-machine_atomic_impl/crate.verus.rs"),
    )
    .unwrap();
    assert!(generated_primitive.contains("vstd::atomic::PAtomicU64::new(value)"));
    assert!(generated_primitive.contains("atomic.load(Tracked(&permission))"));
    assert!(!generated_primitive.contains("external_body"));

    let consumer = temp.0.join("machine-atomic-consumer");
    let linked = codegen_rustc(&bundle)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-composition/machine_atomic_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!(
            "thermite_machine_atomic={}",
            bundle
                .join("artifact/libthermite_machine_atomic.rlib")
                .display()
        ))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            bundle.join("artifact/deps").display()
        ))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(&consumer)
        .output()
        .unwrap();
    assert_success(&linked);
    assert_success(&Command::new(&consumer).output().unwrap());

    for (name, relative) in [
        (
            "machine-model",
            "evidence/machine-models/pinned-vstd-atomic.rs",
        ),
        ("machine-rlib", "artifact/deps/libvstd.rlib"),
        (
            "machine-object",
            "artifact/deps/libmachine_atomic_impl.rlib",
        ),
    ] {
        let tampered = temp.0.join(format!("tampered-{name}.verified"));
        copy_tree(&bundle, &tampered);
        let path = tampered.join(relative);
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b' ');
        fs::write(path, bytes).unwrap();
        assert!(
            !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
                .status
                .success(),
            "tampered machine evidence `{name}` was accepted"
        );
    }

    let substituted_source =
        b"pub fn atomic_roundtrip_impl(value: u64) -> (result: u64) ensures result == value, { value }\n";
    let substituted_path = temp.0.join("substituted-machine.rs");
    fs::write(&substituted_path, substituted_source).unwrap();
    let mut substituted_registry: serde_json::Value = serde_json::from_slice(
        &fs::read(root().join("conformance/verified-composition/machine_atomic_registry.json"))
            .unwrap(),
    )
    .unwrap();
    substituted_registry["entries"][0]["implementation"]["shell_module"] =
        serde_json::json!("substituted_machine");
    substituted_registry["entries"][0]["implementation"]["source_sha256"] =
        serde_json::json!(digest(substituted_source));
    substituted_registry["entries"][0]["implementation"]["symbol"] =
        serde_json::json!("substituted_machine::atomic_roundtrip_impl");
    let substituted_registry_path = temp.0.join("substituted-registry.json");
    fs::write(
        &substituted_registry_path,
        serde_json::to_vec_pretty(&substituted_registry).unwrap(),
    )
    .unwrap();
    let rejected_bundle = temp.0.join("substituted-must-not-publish.verified");
    let rejected = forge(&[
        "build",
        "conformance/verified-composition/machine_atomic.th",
        "--level",
        "l3",
        "--compose-export",
        "machine_atomic_observation",
        "--compose-shell",
        substituted_path.to_string_lossy().as_ref(),
        "--primitive-registry",
        substituted_registry_path.to_string_lossy().as_ref(),
        "--crate-name",
        "thermite_substituted_machine_atomic",
        "--target",
        "kernel",
        "--out",
        rejected_bundle.to_string_lossy().as_ref(),
        "--json",
    ]);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(!rejected_bundle.exists());
    assert!(String::from_utf8_lossy(&rejected.stdout).contains("canonical pinned-vstd atomic"));
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
    let tv_rows = tv["rows"].as_array().unwrap();
    assert!(tv_rows.iter().any(|row| {
        row["phase"] == "contract"
            && row["label"] == "probe_step.ens#4"
            && row["verdict"] == "faithful"
    }));
    assert!(tv_rows.iter().any(|row| {
        row["phase"] == "body" && row["label"] == "probe_step" && row["verdict"] == "faithful"
    }));
    assert!(!tv_rows
        .iter()
        .any(|row| row["phase"] == "exec" && row["label"] == "probe_step.tail"));

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

/// The synthetic test platform.
///
/// `.design/build/kernel-primitives.md` §"Acceptance matrix": "Compose a
/// synthetic test platform whose bodies are tiny direct-Verus adapters. This
/// exercises the registry/refinement machinery without booting or implementing
/// a kernel."
///
/// `conformance/verified-composition/synthetic_platform.th` mirrors two tracked
/// declarations from `stdlib/kernel-primitives/platform/api.th`,
/// `fn raw_address_from_region` and `fn raw_address_advance`. Mirroring their
/// declaration shape carries the sealed ownership transition that
/// `.design/build/frozen-primitive-registry.md` §"Schema v1" requires every
/// entry to declare — "parameter and result ownership (`by_value`,
/// shared/exclusive borrow, sealed consume, or sealed mint) derived
/// independently from the AST" — which the `by_value` scalar fixtures do not
/// reach. The mirror stops at the effect row: the synthetic doors declare
/// `fx pure`, so their source-derived machine class is `sequential`
/// (§"Source-derived minimum machine class") and the safe v1 linkage admits
/// them, where the mirrored platform doors carry `platform(memory)` and reject.
///
/// R-CHAR-3: the expected ownership rows are derived from the tracked
/// `platform/api.th` declarations and the design vocabulary above, never from
/// forge's output. The three content digests in the registry fixture hash the
/// AST debug form and are covered by the drift battery at the end of this test.
#[test]
fn synthetic_platform_composes_the_sealed_ownership_transition() {
    let api = read_program("stdlib/kernel-primitives/platform/api.th");
    let api_sealed = sealed_type_names(&api);
    let fixture = read_program("conformance/verified-composition/synthetic_platform.th");
    let fixture_sealed = sealed_type_names(&fixture);

    let registry_bytes =
        fs::read(root().join("conformance/verified-composition/synthetic_platform_registry.json"))
            .unwrap();
    let registry: serde_json::Value = serde_json::from_slice(&registry_bytes).unwrap();
    assert_eq!(registry["schema"], "thermite.frozen-primitive-registry.v1");

    // Each synthetic door reproduces a tracked platform door's ownership row.
    let mut declared_vocabulary = std::collections::BTreeSet::new();
    for (tracked_name, synthetic_name) in [
        ("raw_address_from_region", "syn_address_from_region"),
        ("raw_address_advance", "syn_address_advance"),
    ] {
        let tracked = boundary_named(&api, tracked_name);
        let synthetic = boundary_named(&fixture, synthetic_name);

        let expected_parameters: Vec<&str> = tracked
            .params
            .iter()
            .map(|parameter| declared_parameter_ownership(&parameter.ty, &api_sealed))
            .collect();
        let expected_result = declared_result_ownership(&tracked.ret, &api_sealed);
        let found_parameters: Vec<&str> = synthetic
            .params
            .iter()
            .map(|parameter| declared_parameter_ownership(&parameter.ty, &fixture_sealed))
            .collect();
        let found_result = declared_result_ownership(&synthetic.ret, &fixture_sealed);
        assert_eq!(
            found_parameters, expected_parameters,
            "`{synthetic_name}` must mirror the parameter ownership of tracked platform door \
             `{tracked_name}`"
        );
        assert_eq!(
            found_result, expected_result,
            "`{synthetic_name}` must mirror the result ownership of tracked platform door \
             `{tracked_name}`"
        );

        // `.design/build/frozen-primitive-registry.md` §"Source-derived minimum
        // machine class": "`sequential` stays reachable through the empty
        // maximum: a `#[boundary]` whose effect row carries no platform atom."
        let entry = registry["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["thermite_name"] == synthetic_name)
            .unwrap_or_else(|| panic!("registry declares no entry for `{synthetic_name}`"));
        assert_eq!(entry["effects"], serde_json::json!(["pure"]));
        assert_eq!(entry["concurrency"], "sequential");
        assert_eq!(
            entry["ownership"]["parameters"],
            serde_json::json!(expected_parameters)
        );
        assert_eq!(entry["ownership"]["result"], expected_result);
        for parameter in &expected_parameters {
            declared_vocabulary.insert((*parameter).to_string());
        }
        declared_vocabulary.insert(expected_result.to_string());
    }
    for required in ["shared_borrow", "consume_sealed", "mint_sealed"] {
        assert!(
            declared_vocabulary.contains(required),
            "the synthetic platform must compose ownership `{required}`; it declares \
             {declared_vocabulary:?}"
        );
    }

    let tracked_shell =
        PathBuf::from("conformance/verified-composition/synthetic_platform_shell.rs");
    let tracked_registry =
        PathBuf::from("conformance/verified-composition/synthetic_platform_registry.json");
    let temp = TempDir::new("synthetic-platform");
    let bundle = temp.0.join("synthetic.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    let args = synthetic_build_args(
        &tracked_shell,
        &tracked_registry,
        "thermite_synthetic_platform",
        &bundle,
    );
    assert_success(&forge(&refs(&args)));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay"]));

    // §"Exact checked-wrapper refinement": the generated boundary function "has
    // a real body. It carries no `external_body`, `assume`, `admit`, `unsafe`,
    // or `unimplemented` exemption."
    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains(
        "fn syn_address_from_region(region: &SynRegion, offset: usize) -> (result: SynAddress)"
    ));
    assert!(
        source.contains("synthetic_platform_shell::syn_address_from_region_impl(region, offset)")
    );
    assert!(source.contains(
        "fn syn_address_advance(address: SynAddress, length: usize) -> (result: SynAddress)"
    ));
    assert!(source.contains("synthetic_platform_shell::syn_address_advance_impl(address, length)"));
    assert!(!source.contains("external_body"));
    assert!(!source.contains("unimplemented!"));

    // The ownership vocabulary reaches the frozen plan, not just the input JSON.
    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    let entries = plan["composition"]["primitive_registry"]["entries"]
        .as_array()
        .unwrap();
    assert_eq!(entries.len(), 2);
    let mut planned_vocabulary = std::collections::BTreeSet::new();
    for entry in entries {
        assert_eq!(entry["reachable"], true);
        assert_eq!(entry["implementation_linkage"], "same_crate");
        assert_eq!(entry["refinement"], "same_crate_verus_checked_wrapper");
        for parameter in entry["parameter_ownership"].as_array().unwrap() {
            planned_vocabulary.insert(parameter.as_str().unwrap().to_string());
        }
        planned_vocabulary.insert(entry["result_ownership"].as_str().unwrap().to_string());
    }
    assert_eq!(planned_vocabulary, declared_vocabulary);

    // §"Receipt and replay" plus §"Exact checked-wrapper refinement": a safe
    // sequential door reaches `L3-direct-refinement` and carries no residual
    // machine assumption. Three mandatory `same_crate` obligations per entry
    // (§"Schema v1") over two reachable entries is six discharged obligations.
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    let binding = &receipt["binding"];
    assert_eq!(binding["assurance"], "L3");
    assert_eq!(binding["scope"], "end_to_end");
    assert_eq!(binding["composition"]["reachable_primitive_count"], 2);
    assert_eq!(
        binding["composition"]["discharged_refinement_obligations"],
        6
    );
    assert_eq!(binding["composition"]["residual_machine_assumptions"], 0);
    let members = binding["assurance_aggregate"]["members"]
        .as_array()
        .unwrap();
    for door in ["syn_address_from_region", "syn_address_advance"] {
        assert!(
            members.iter().any(|member| {
                member["name"] == door
                    && member["kind"] == "frozen_primitive_boundary"
                    && member["achieved"] == "L3-direct-refinement"
            }),
            "`{door}` must publish as a directly refined frozen primitive boundary; \
             members = {members:?}"
        );
    }

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    assert!(tv["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["verdict"] == "faithful"));

    // §"Receipt and replay": "Registry byte tampering ... rejects."
    let tampered = temp.0.join("tampered.verified");
    copy_tree(&bundle, &tampered);
    let tampered_registry = tampered.join("evidence/frozen-primitive-registry.json");
    let mut bytes = fs::read(&tampered_registry).unwrap();
    bytes.push(b' ');
    fs::write(&tampered_registry, bytes).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success()
    );

    // §Acceptance: "Signature, contract, effect, target, ABI, ownership,
    // shell-source, proof-list, and schema drift fail closed." The three content
    // digests are hashes over the AST debug form, so this battery is what keeps
    // them honest.
    let drift_dir = temp.0.join("drift");
    fs::create_dir(&drift_dir).unwrap();
    for (index, (field, mutate, fragment)) in [
        (
            "contract_sha256",
            &(|entry: &mut serde_json::Value| {
                entry["contract_sha256"] = serde_json::json!("0".repeat(64));
            }) as &dyn Fn(&mut serde_json::Value),
            "contract_sha256 drift",
        ),
        (
            "effects_sha256",
            &|entry: &mut serde_json::Value| {
                entry["effects_sha256"] = serde_json::json!("0".repeat(64));
            },
            "effects_sha256 drift",
        ),
        (
            "implementation.source_sha256",
            &|entry: &mut serde_json::Value| {
                entry["implementation"]["source_sha256"] = serde_json::json!("0".repeat(64));
            },
            "implementation.source_sha256 drift",
        ),
        (
            "signature",
            &|entry: &mut serde_json::Value| {
                entry["signature"] = serde_json::json!(
                    "fn syn_address_from_region(region:SynRegion,offset:usize)->SynAddress"
                );
            },
            "signature drift",
        ),
        (
            "ownership",
            &|entry: &mut serde_json::Value| {
                entry["ownership"]["parameters"] = serde_json::json!(["by_value", "by_value"]);
            },
            "ownership drift",
        ),
        (
            "result_ownership",
            &|entry: &mut serde_json::Value| {
                entry["ownership"]["result"] = serde_json::json!("by_value");
            },
            "ownership drift",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut drifted: serde_json::Value = serde_json::from_slice(&registry_bytes).unwrap();
        let target = drifted["entries"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["thermite_name"] == "syn_address_from_region")
            .unwrap();
        mutate(target);
        let drifted_path = drift_dir.join(format!("drift-{index}.registry.json"));
        fs::write(&drifted_path, serde_json::to_vec_pretty(&drifted).unwrap()).unwrap();
        let drifted_bundle = temp
            .0
            .join(format!("drift-{index}-must-not-publish.verified"));
        let mut args = synthetic_build_args(
            &tracked_shell,
            &drifted_path,
            "thermite_synthetic_platform_drift",
            &drifted_bundle,
        );
        args.push("--json".to_string());
        let rejected = forge(&refs(&args));
        assert_eq!(
            rejected.status.code(),
            Some(1),
            "`{field}` drift was accepted: {}",
            String::from_utf8_lossy(&rejected.stdout)
        );
        assert!(
            !drifted_bundle.exists(),
            "`{field}` drift published a bundle"
        );
        assert!(
            String::from_utf8_lossy(&rejected.stdout).contains(fragment),
            "`{field}` drift diagnostic must name `{fragment}`: {}",
            String::from_utf8_lossy(&rejected.stdout)
        );
    }

    // §Acceptance: "A digest-updated shell whose body violates the Thermite
    // contract reaches the real whole-crate proof, fails there, and publishes
    // nothing." The adapter drops the advance, so the mint no longer refines.
    let lying_shell = fs::read_to_string(
        root().join("conformance/verified-composition/synthetic_platform_shell.rs"),
    )
    .unwrap()
    .replace(
        "offset: address.offset + length,",
        "offset: address.offset,",
    );
    let lying_shell_path = drift_dir.join("synthetic_platform_shell.rs");
    fs::write(&lying_shell_path, &lying_shell).unwrap();
    let mut lying_registry: serde_json::Value = serde_json::from_slice(&registry_bytes).unwrap();
    for entry in lying_registry["entries"].as_array_mut().unwrap() {
        entry["implementation"]["source_sha256"] =
            serde_json::json!(digest(lying_shell.as_bytes()));
    }
    let lying_registry_path = drift_dir.join("lying.registry.json");
    fs::write(
        &lying_registry_path,
        serde_json::to_vec_pretty(&lying_registry).unwrap(),
    )
    .unwrap();
    let lying_bundle = temp.0.join("lying-must-not-publish.verified");
    let mut lying_args = synthetic_build_args(
        &lying_shell_path,
        &lying_registry_path,
        "thermite_synthetic_platform_lie",
        &lying_bundle,
    );
    lying_args.push("--json".to_string());
    let lying = forge(&refs(&lying_args));
    assert_eq!(lying.status.code(), Some(1));
    assert!(!lying_bundle.exists());
    assert!(String::from_utf8_lossy(&lying.stdout).contains("whole-crate-verus"));
}

fn synthetic_build_args(
    shell: &Path,
    registry: &Path,
    crate_name: &str,
    bundle: &Path,
) -> Vec<String> {
    vec![
        "build".to_string(),
        "conformance/verified-composition/synthetic_platform.th".to_string(),
        "--level".to_string(),
        "l3".to_string(),
        "--compose-export".to_string(),
        "syn_platform_observation".to_string(),
        "--compose-shell".to_string(),
        shell.to_string_lossy().to_string(),
        "--primitive-registry".to_string(),
        registry.to_string_lossy().to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "--target".to_string(),
        "kernel".to_string(),
        "--out".to_string(),
        bundle.to_string_lossy().to_string(),
    ]
}

fn read_program(relative: &str) -> thermite_syntax::Program {
    let source = fs::read_to_string(root().join(relative)).unwrap();
    let parsed = thermite_syntax::parse(&source);
    assert!(parsed.is_clean(), "{relative}: {:?}", parsed.errors);
    parsed.program
}

fn sealed_type_names(program: &thermite_syntax::Program) -> std::collections::BTreeSet<String> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            thermite_syntax::Item::Struct(item) if item.sealed => Some(item.name.clone()),
            _ => None,
        })
        .collect()
}

fn boundary_named<'a>(
    program: &'a thermite_syntax::Program,
    name: &str,
) -> &'a thermite_syntax::FnItem {
    program
        .items
        .iter()
        .find_map(|item| match item {
            thermite_syntax::Item::Fn(function)
                if function.name == name && function.boundary.is_some() =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no #[boundary] declaration named `{name}`"))
}

/// The parameter half of the ownership vocabulary
/// `.design/build/frozen-primitive-registry.md` §"Schema v1" freezes:
/// "`by_value`, shared/exclusive borrow, sealed consume, or sealed mint".
fn declared_parameter_ownership(
    ty: &thermite_syntax::Type,
    sealed: &std::collections::BTreeSet<String>,
) -> &'static str {
    match ty {
        thermite_syntax::Type::Ref { mutable: true, .. } => "exclusive_borrow",
        thermite_syntax::Type::Ref { mutable: false, .. } => "shared_borrow",
        _ if mentions_sealed(ty, sealed) => "consume_sealed",
        _ => "by_value",
    }
}

/// The result half of the same vocabulary. A borrowed return type is outside
/// it, so this reports the absent case rather than inventing a spelling.
fn declared_result_ownership(
    ty: &thermite_syntax::Type,
    sealed: &std::collections::BTreeSet<String>,
) -> &'static str {
    match ty {
        thermite_syntax::Type::Ref { .. } | thermite_syntax::Type::Slice(_) => "borrowed_result",
        _ if mentions_sealed(ty, sealed) => "mint_sealed",
        _ => "by_value",
    }
}

fn mentions_sealed(
    ty: &thermite_syntax::Type,
    sealed: &std::collections::BTreeSet<String>,
) -> bool {
    use thermite_syntax::Type;
    match ty {
        Type::Named(name) => sealed.contains(name),
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Array { elem: inner, .. }
        | Type::Generic { arg: inner, .. }
        | Type::Box(inner)
        | Type::Vec(inner)
        | Type::Option(inner) => mentions_sealed(inner, sealed),
        Type::Result(ok, err) | Type::Map(ok, err) => {
            mentions_sealed(ok, sealed) || mentions_sealed(err, sealed)
        }
        Type::Tuple(items) => items.iter().any(|item| mentions_sealed(item, sealed)),
        Type::Prim(_) | Type::Unit | Type::String => false,
    }
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
