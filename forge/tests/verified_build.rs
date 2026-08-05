//! Real-toolchain conformance for correspondence-backed L3 artifacts.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const INCOMPATIBLE_RUSTUP_TOOLCHAIN: &str = "1.96.0-x86_64-unknown-linux-gnu";

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

fn forge_with_incompatible_host_rustc(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(args)
        .current_dir(root())
        .env("RUSTUP_TOOLCHAIN", INCOMPATIBLE_RUSTUP_TOOLCHAIN)
        .output()
        .unwrap()
}

fn toolchain_evidence(bundle: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(bundle.join("evidence/toolchain.json")).unwrap()).unwrap()
}

fn codegen_rustup_toolchain(bundle: &Path) -> String {
    toolchain_evidence(bundle)["artifact_codegen"]["rustup_toolchain"]
        .as_str()
        .unwrap()
        .to_string()
}

fn codegen_rustc(bundle: &Path) -> Command {
    let mut command = Command::new("rustup");
    command.args(["run", &codegen_rustup_toolchain(bundle), "rustc"]);
    command
}

fn incompatible_rustc() -> Command {
    let mut command = Command::new("rustup");
    command.args(["run", INCOMPATIBLE_RUSTUP_TOOLCHAIN, "rustc"]);
    command
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_verified_build_test_{name}_{}_{}",
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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

fn rewrite_json(path: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    mutate(&mut value);
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

#[test]
fn hosted_bundle_is_exact_private_linkable_tamper_evident_and_reproducible() {
    let temp = TempDir::new("hosted");
    let bundle_a = temp.0.join("a.verified");
    let bundle_b = temp.0.join("b.verified");
    let bundle_a_s = bundle_a.to_string_lossy().to_string();
    let bundle_b_s = bundle_b.to_string_lossy().to_string();
    assert_success(&forge_with_incompatible_host_rustc(&[
        "build",
        "conformance/verified-build/identity.th",
        "--level",
        "l3",
        "--export",
        "identity",
        "--crate-name",
        "deep_identity",
        "--out",
        &bundle_a_s,
        "--json",
    ]));
    assert_success(&forge_with_incompatible_host_rustc(&[
        "verify-build",
        &bundle_a_s,
        "--replay",
        "--json",
    ]));

    let toolchain = toolchain_evidence(&bundle_a);
    assert_eq!(
        toolchain["artifact_codegen"]["rustup_toolchain"],
        "1.95.0-x86_64-unknown-linux-gnu"
    );
    assert!(toolchain["artifact_codegen"]["rustc_version"]
        .as_str()
        .unwrap()
        .contains("release: 1.95.0"));
    assert!(toolchain["host_rustc"]["rustc_version"]
        .as_str()
        .unwrap()
        .contains("release: 1.96.0"));
    assert!(toolchain["artifact_codegen"]["supported_target_features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|feature| feature == "sse2"));
    assert_eq!(
        toolchain["environment"]["RUSTUP_TOOLCHAIN"],
        toolchain["artifact_codegen"]["rustup_toolchain"]
    );
    for field in [
        "rustc_sha256",
        "rustc_driver_sha256",
        "llvm_library_sha256",
        "target_libdir_sha256",
    ] {
        assert_eq!(
            toolchain["artifact_codegen"][field].as_str().unwrap().len(),
            64,
            "missing codegen digest `{field}`"
        );
    }

    let source = fs::read_to_string(bundle_a.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains("pub fn identity"));
    assert!(!source.contains("hidden_identity"));
    assert!(!source.contains("thermite_check!"));
    assert!(!source.contains("external_body"));

    let consumer = temp.0.join("consumer");
    let artifact = bundle_a.join("artifact/libdeep_identity.rlib");
    let link = codegen_rustc(&bundle_a)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-build/host_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!("deep_identity={}", artifact.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            bundle_a.join("artifact/deps").display()
        ))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(&consumer)
        .output()
        .unwrap();
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let incompatible = incompatible_rustc()
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-build/host_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!("deep_identity={}", artifact.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            bundle_a.join("artifact/deps").display()
        ))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(temp.0.join("incompatible-consumer"))
        .output()
        .unwrap();
    assert!(!incompatible.status.success());
    let incompatible_stderr = String::from_utf8_lossy(&incompatible.stderr);
    assert!(
        incompatible_stderr.contains("incompatible version of rustc")
            && incompatible_stderr.contains("compiled by rustc 1.95.0"),
        "{incompatible_stderr}"
    );

    assert_success(&forge_with_incompatible_host_rustc(&[
        "build",
        "conformance/verified-build/identity.th",
        "--level",
        "l3",
        "--export",
        "identity",
        "--crate-name",
        "deep_identity",
        "--out",
        &bundle_b_s,
    ]));
    assert_eq!(
        fs::read(bundle_a.join("receipt.json")).unwrap(),
        fs::read(bundle_b.join("receipt.json")).unwrap()
    );
    assert_eq!(
        fs::read(&artifact).unwrap(),
        fs::read(bundle_b.join("artifact/libdeep_identity.rlib")).unwrap()
    );

    for (index, relative) in [
        "evidence/input.th",
        "evidence/artifact-plan.v1",
        "evidence/source.verus.rs",
        "evidence/certificates.json",
        "evidence/translation-validation.json",
        "evidence/verus-result.json",
        "evidence/toolchain.json",
        "artifact/libdeep_identity.rlib",
    ]
    .iter()
    .enumerate()
    {
        let tampered = temp.0.join(format!("tampered-{index}.verified"));
        copy_tree(&bundle_a, &tampered);
        let path = tampered.join(relative);
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b' ');
        fs::write(&path, bytes).unwrap();
        let output = forge(&["verify-build", tampered.to_string_lossy().as_ref()]);
        assert!(
            !output.status.success(),
            "tampering `{relative}` was accepted"
        );
    }

    for (name, relative, mutate) in [
        (
            "closure-row",
            "evidence/artifact-plan.v1",
            (|value: &mut serde_json::Value| {
                value["closure_nodes"][0]["semantic_address"] =
                    serde_json::Value::String("fn::tampered".to_string());
            }) as fn(&mut serde_json::Value),
        ),
        (
            "strict-flag",
            "evidence/artifact-plan.v1",
            (|value: &mut serde_json::Value| {
                value["expected_verus_args"][0] =
                    serde_json::Value::String("--tampered".to_string());
            }) as fn(&mut serde_json::Value),
        ),
        (
            "abi-row",
            "receipt.json",
            (|value: &mut serde_json::Value| {
                value["binding"]["exports"][0]["abi_sha256"] =
                    serde_json::Value::String("0".repeat(64));
            }) as fn(&mut serde_json::Value),
        ),
        (
            "tool-identity",
            "evidence/toolchain.json",
            (|value: &mut serde_json::Value| {
                value["verus_sha256"] = serde_json::Value::String("0".repeat(64));
            }) as fn(&mut serde_json::Value),
        ),
        (
            "codegen-rustc-identity",
            "evidence/toolchain.json",
            (|value: &mut serde_json::Value| {
                value["artifact_codegen"]["rustup_toolchain"] =
                    serde_json::Value::String(INCOMPATIBLE_RUSTUP_TOOLCHAIN.to_string());
            }) as fn(&mut serde_json::Value),
        ),
        (
            "codegen-rustc-digest",
            "evidence/toolchain.json",
            (|value: &mut serde_json::Value| {
                value["artifact_codegen"]["rustc_sha256"] =
                    serde_json::Value::String("0".repeat(64));
            }) as fn(&mut serde_json::Value),
        ),
    ] {
        let tampered = temp.0.join(format!("semantic-{name}.verified"));
        copy_tree(&bundle_a, &tampered);
        rewrite_json(&tampered.join(relative), mutate);
        assert!(
            !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
                .status
                .success(),
            "semantic tampering `{name}` was accepted"
        );
    }

    let extra = temp.0.join("extra.verified");
    copy_tree(&bundle_a, &extra);
    fs::write(extra.join("evidence/unbound.log"), b"unbound").unwrap();
    assert!(!forge(&["verify-build", extra.to_string_lossy().as_ref()])
        .status
        .success());

    let existing = temp.0.join("existing.verified");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("marker"), b"preserve").unwrap();
    let output = forge(&[
        "build",
        "conformance/verified-build/identity.th",
        "--level",
        "l3",
        "--export",
        "identity",
        "--out",
        existing.to_string_lossy().as_ref(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(fs::read(existing.join("marker")).unwrap(), b"preserve");
}

#[test]
fn fixed_array_logic_is_compiled_and_bound_by_all_strict_l3_gates() {
    let temp = TempDir::new("fixed-array");
    let bundle = temp.0.join("fixed-array.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/fixed_array.th",
        "--level",
        "l3",
        "--export",
        "fixed_array_read",
        "--target",
        "kernel",
        "--crate-name",
        "fixed_array_read",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n#![crate_type = \"rlib\"]"));
    assert!(source.contains("use vstd::prelude::*;"));
    assert!(source.contains("pub const SLOTS: usize = 4;"), "{source}");
    assert!(
        source.contains(
            "let slots: [u64; SLOTS] = vstd::array::array_fill_for_copy_types::<_, SLOTS>(7);"
        ),
        "{source}"
    );
    assert!(source.contains("slots[at]"), "{source}");
    assert!(
        source.contains("pub trait __thermite_FixedArrayEq"),
        "{source}"
    );
    assert!(source.contains("__thermite_fixed_array_eq"), "{source}");

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["target"], "kernel");
    for expected in [
        "--no-vstd",
        "vstd=<KERNEL_VSTD_VIR>",
        "vstd=<KERNEL_VSTD_RLIB>",
    ] {
        assert!(
            plan["expected_verus_args"]
                .as_array()
                .unwrap()
                .iter()
                .any(|arg| arg == expected),
            "missing `{expected}`: {plan}"
        );
    }

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "every reachable array contract/expression/body/wrapper row must be faithful: {tv}"
    );
}

#[test]
fn aggregate_array_relations_are_exported_replayed_and_tamper_evident() {
    let temp = TempDir::new("aggregate-array-relations");
    let bundle = temp.0.join("aggregate-array-relations.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/aggregate_array_relations.th",
        "--level",
        "l3",
        "--export",
        "aggregate_array_equal",
        "--crate-name",
        "aggregate_array_relations",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(
        source.contains("fn __thermite_element_eq_Struct_Stamp"),
        "{source}"
    );
    assert!(
        source.contains("fn __thermite_element_eq_Struct_Slot"),
        "{source}"
    );
    assert!(
        source.contains("__thermite_FixedArrayEq for [Slot; N]"),
        "{source}"
    );
    assert!(source.contains("(left.owner) == (right.owner)"), "{source}");

    let consumer_source = temp.0.join("aggregate-array-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use aggregate_array_relations::{aggregate_array_equal, Slot, Stamp};

fn slot(owner: usize) -> Slot {
    Slot { stamp: Stamp { words: [3, 5], flags: (true, 7) }, owner }
}

fn main() {
    let left: [Slot; 4] = std::array::from_fn(slot);
    let right: [Slot; 4] = std::array::from_fn(slot);
    assert!(aggregate_array_equal(left, right));
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("aggregate-array-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "aggregate_array_relations={}",
            bundle
                .join("artifact/libaggregate_array_relations.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(
        receipt["binding"]["exports"][0]["parameter_types"],
        serde_json::json!(["[Slot;SLOTS]", "[Slot;SLOTS]"])
    );
    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        4,
        "contract, exec, and body rows are mandatory: {tv}"
    );
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "every aggregate relation row must be faithful: {tv}"
    );

    let tampered = temp.0.join("aggregate-array-relations-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen("owner: usize", "owner: u64", 1);
    assert_ne!(
        changed, original,
        "tamper fixture must alter the bound record field"
    );
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound aggregate representation must invalidate the receipt"
    );
}

#[test]
fn named_record_lifecycle_is_exported_replayed_and_executes_generated_logic() {
    let temp = TempDir::new("named-record-lifecycle");
    let bundle = temp.0.join("named-record-lifecycle.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/named_record_lifecycle.th",
        "--level",
        "l3",
        "--export",
        "named_record_advance",
        "--export",
        "named_record_new",
        "--export",
        "named_record_generation",
        "--export",
        "named_record_occupied",
        "--crate-name",
        "named_record_lifecycle",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains("pub fn named_record_advance"), "{source}");
    assert!(source.contains("pub fn named_record_new"), "{source}");
    assert!(source.contains("state.generation = next;"), "{source}");
    assert!(source.contains("pub(crate) generation: u64"), "{source}");
    assert!(source.contains("pub(crate) occupied: bool"), "{source}");
    assert!(
        source.contains(
            "named_record_occupied_spec(*final(state)) == named_record_occupied_spec(*old(state))"
        ),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("named-record-lifecycle-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use named_record_lifecycle::{
    named_record_advance, named_record_generation, named_record_new, named_record_occupied,
};

fn main() {
    let mut state = named_record_new(7, true);
    let previous = named_record_advance(&mut state, 11);
    assert!(previous);
    assert_eq!(named_record_generation(&state), 11);
    assert!(named_record_occupied(&state));
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("named-record-lifecycle-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "named_record_lifecycle={}",
            bundle
                .join("artifact/libnamed_record_lifecycle.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let privacy_source = temp.0.join("named-record-lifecycle-private.rs");
    fs::write(
        &privacy_source,
        r#"
use named_record_lifecycle::named_record_new;

fn main() {
    let state = named_record_new(7, true);
    let _ = state.generation;
}
"#,
    )
    .unwrap();
    let privacy = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&privacy_source)
        .arg("--extern")
        .arg(format!(
            "named_record_lifecycle={}",
            bundle
                .join("artifact/libnamed_record_lifecycle.rlib")
                .display()
        ))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            bundle.join("artifact/deps").display()
        ))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(temp.0.join("must-not-compile-private-record-access"))
        .output()
        .unwrap();
    assert!(
        !privacy.status.success()
            && String::from_utf8_lossy(&privacy.stderr).contains("private field"),
        "opaque record fields became externally visible: {}",
        String::from_utf8_lossy(&privacy.stderr)
    );

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(
        receipt["binding"]["exports"][0]["parameter_types"],
        serde_json::json!(["&mut State", "u64"])
    );
    assert_eq!(
        receipt["binding"]["exports"][0]["ownership"],
        serde_json::json!(["exclusive_borrow", "by_value"])
    );
    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        20,
        "constructor, observers, and mutator require twenty contract/exec/body rows: {tv}"
    );
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "every lifecycle proof row must be faithful: {tv}"
    );

    let tampered = temp.0.join("named-record-lifecycle-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen("generation: u64", "generation: usize", 1);
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound record layout must invalidate the lifecycle receipt"
    );
}

#[test]
fn owned_aggregate_pipeline_is_strict_freestanding_l3_and_executes_generated_logic() {
    let temp = TempDir::new("owned-aggregate-lifecycle");
    let bundle = temp.0.join("owned-aggregate-lifecycle.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/owned_aggregate_lifecycle.th",
        "--level",
        "l3",
        "--export",
        "owned_state_pipeline",
        "--export",
        "owned_state_generation",
        "--export",
        "owned_state_occupied",
        "--export",
        "owned_state_first",
        "--export",
        "owned_state_second",
        "--crate-name",
        "owned_aggregate_lifecycle",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(
        source.contains("let mut updated: OwnedState = state;"),
        "{source}"
    );
    assert!(source.contains("updated.generation = mixed;"), "{source}");
    assert!(source.contains("updated.second = mixed;"), "{source}");
    assert!(source.contains("fn owned_state_pipeline"), "{source}");
    assert!(
        source.contains("pub fn thermite_export_owned_state_pipeline_v1"),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("owned-aggregate-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use owned_aggregate_lifecycle::{
    owned_state_first, owned_state_generation, owned_state_occupied, owned_state_second,
    thermite_export_owned_state_pipeline_v1,
};

fn main() {
    let state = match thermite_export_owned_state_pipeline_v1(11, 29) {
        Ok(state) => state,
        Err(_) => panic!("valid generated pipeline inputs were rejected"),
    };
    assert_eq!(owned_state_generation(&state), 11);
    assert!(owned_state_occupied(&state));
    assert_eq!(owned_state_first(&state), 3);
    assert_eq!(owned_state_second(&state), 29);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("owned-aggregate-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "owned_aggregate_lifecycle={}",
            bundle
                .join("artifact/libowned_aggregate_lifecycle.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert_eq!(receipt["binding"]["target"], "kernel");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "strict aggregate lifecycle admitted a non-faithful row: {tv}"
    );
    for (phase, label) in [
        ("body", "owned_state_pipeline"),
        ("exec", "owned_state_pipeline.let#2"),
        ("exec", "owned_state_pipeline.tail"),
        ("wrapper_guard", "owned_state_pipeline.export_guard"),
    ] {
        assert!(
            rows.iter().any(|row| {
                row["phase"] == phase && row["label"] == label && row["verdict"] == "faithful"
            }),
            "missing {phase} L3 row `{label}`: {tv}"
        );
    }

    let tampered = temp.0.join("owned-aggregate-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen("first: u64", "first: usize", 1);
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound owned-record layout must invalidate the receipt"
    );
}

#[test]
fn nested_aggregate_pipeline_is_strict_freestanding_l3_and_executes_generated_logic() {
    let temp = TempDir::new("nested-aggregate-lifecycle");
    let bundle = temp.0.join("nested-aggregate-lifecycle.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/nested_aggregate_lifecycle.th",
        "--level",
        "l3",
        "--export",
        "nested_state_pipeline",
        "--export",
        "nested_state_value",
        "--export",
        "nested_state_guard",
        "--export",
        "nested_state_tag",
        "--crate-name",
        "nested_aggregate_lifecycle",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(
        source.contains("let mut updated: NestedState = state;"),
        "{source}"
    );
    assert!(source.contains("updated.inner.value = next;"), "{source}");
    assert!(source.contains("fn nested_state_pipeline"), "{source}");
    assert!(
        source.contains("pub fn thermite_export_nested_state_pipeline_v1"),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("nested-aggregate-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use nested_aggregate_lifecycle::{
    nested_state_guard, nested_state_tag, nested_state_value,
    thermite_export_nested_state_pipeline_v1,
};

fn main() {
    let state = match thermite_export_nested_state_pipeline_v1(3, 5, 7, 11) {
        Ok(state) => state,
        Err(_) => panic!("valid generated nested pipeline inputs were rejected"),
    };
    assert_eq!(nested_state_value(&state), 11);
    assert_eq!(nested_state_guard(&state), 5);
    assert_eq!(nested_state_tag(&state), 7);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("nested-aggregate-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "nested_aggregate_lifecycle={}",
            bundle
                .join("artifact/libnested_aggregate_lifecycle.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert_eq!(receipt["binding"]["target"], "kernel");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "strict nested lifecycle admitted a non-faithful row: {tv}"
    );
    for (phase, label) in [
        ("body", "nested_state_update"),
        ("body", "nested_state_pipeline"),
        ("exec", "nested_state_update.let#1"),
        ("exec", "nested_state_pipeline.tail"),
        ("wrapper_guard", "nested_state_pipeline.export_guard"),
    ] {
        assert!(
            rows.iter().any(|row| {
                row["phase"] == phase && row["label"] == label && row["verdict"] == "faithful"
            }),
            "missing {phase} L3 row `{label}`: {tv}"
        );
    }

    let tampered = temp.0.join("nested-aggregate-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen("value: u64", "value: usize", 1);
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound nested-record layout must invalidate the receipt"
    );
}

#[test]
fn record_state_loop_is_strict_freestanding_l3_and_executes_generated_loop() {
    let temp = TempDir::new("record-state-loop");
    let bundle = temp.0.join("record-state-loop.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/record_state_loop.th",
        "--level",
        "l3",
        "--export",
        "record_loop",
        "--export",
        "record_loop_cursor",
        "--export",
        "record_loop_total",
        "--export",
        "record_loop_guard",
        "--export",
        "record_loop_tag",
        "--crate-name",
        "record_state_loop",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(source.contains("while state.cursor < limit"), "{source}");
    assert!(
        source.contains("state.inner.total = state.cursor + 1;"),
        "{source}"
    );
    assert!(
        source.contains("state.cursor = state.cursor + 1;"),
        "{source}"
    );
    assert!(source.contains("state.inner.guard == guard"), "{source}");
    assert!(source.contains("state.tag == tag"), "{source}");
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("record-state-loop-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use record_state_loop::{
    record_loop_cursor, record_loop_guard, record_loop_tag, record_loop_total,
    thermite_export_record_loop_v1,
};

fn main() {
    let state = match thermite_export_record_loop_v1(17, 55, 99) {
        Ok(state) => state,
        Err(_) => panic!("valid generated record-loop inputs were rejected"),
    };
    assert_eq!(record_loop_cursor(&state), 17);
    assert_eq!(record_loop_total(&state), 17);
    assert_eq!(record_loop_guard(&state), 55);
    assert_eq!(record_loop_tag(&state), 99);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("record-state-loop-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "record_state_loop={}",
            bundle.join("artifact/librecord_state_loop.rlib").display()
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert_eq!(receipt["binding"]["target"], "kernel");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "strict record-loop lifecycle admitted a non-faithful row: {tv}"
    );
    for (phase, label) in [
        ("loop", "record_loop.loop"),
        ("contract", "record_loop.ens#1"),
        ("contract", "record_loop.ens#4"),
        ("wrapper_guard", "record_loop.export_guard"),
    ] {
        assert!(
            rows.iter().any(|row| {
                row["phase"] == phase && row["label"] == label && row["verdict"] == "faithful"
            }),
            "missing {phase} L3 row `{label}`: {tv}"
        );
    }

    let tampered = temp.0.join("record-state-loop-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen(
        "state.inner.total = state.cursor + 1",
        "state.inner.total = state.cursor + 2",
        1,
    );
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound record-loop step must invalidate the receipt"
    );
}

#[test]
fn mutable_call_effect_is_strict_freestanding_l3_and_executes_generated_calls() {
    let temp = TempDir::new("mutable-call-effect");
    let bundle = temp.0.join("mutable-call-effect.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/mutable_call_effect.th",
        "--level",
        "l3",
        "--export",
        "mutable_call_pipeline",
        "--export",
        "mutable_call_new",
        "--export",
        "mutable_call_value",
        "--export",
        "mutable_call_guard",
        "--crate-name",
        "mutable_call_effect",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(
        source.contains("let initial: u64 = mutable_call_value(source);")
            && source.contains("let first: u64 = mutable_call_copy(state, source);")
            && source.contains("let final_value: u64 = mutable_call_set(state, first);"),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("mutable-call-effect-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use mutable_call_effect::{
    mutable_call_guard, mutable_call_new, mutable_call_pipeline, mutable_call_value,
};

fn main() {
    let mut state = mutable_call_new(3, 77);
    let source = mutable_call_new(41, 88);
    assert_eq!(mutable_call_pipeline(&mut state, &source), 41);
    assert_eq!(mutable_call_value(&state), 41);
    assert_eq!(mutable_call_guard(&state), 77);
    assert_eq!(mutable_call_value(&source), 41);
    assert_eq!(mutable_call_guard(&source), 88);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("mutable-call-effect-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "mutable_call_effect={}",
            bundle
                .join("artifact/libmutable_call_effect.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert_eq!(receipt["binding"]["target"], "kernel");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "strict mutable-call lifecycle admitted a non-faithful row: {tv}"
    );
    assert!(
        rows.iter().any(|row| {
            row["phase"] == "body"
                && row["label"] == "mutable_call_pipeline"
                && row["verdict"] == "faithful"
        }),
        "missing exact mutable-call body row: {tv}"
    );
    for stateful_let in ["mutable_call_pipeline.let#2", "mutable_call_pipeline.let#3"] {
        assert!(
            !rows
                .iter()
                .any(|row| row["phase"] == "exec" && row["label"] == stateful_let),
            "an effectful mutable-call initializer was misclassified as a pure exec expression: {tv}"
        );
    }
    assert!(
        rows.iter().any(|row| {
            row["phase"] == "exec"
                && row["label"] == "mutable_call_pipeline.let#1"
                && row["verdict"] == "faithful"
        }),
        "the pure initializer surrounding mutable calls lost exec TV: {tv}"
    );

    let tampered = temp.0.join("mutable-call-effect-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen(
        "let final_value: u64 = mutable_call_set(state, first)",
        "let final_value: u64 = mutable_call_set(state, first + 1)",
        1,
    );
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound mutable-call effect must invalidate the receipt"
    );
}

#[test]
fn projected_record_call_effect_is_strict_l3_replayed_and_executed() {
    let temp = TempDir::new("projected-record-call-effect");
    let bundle = temp.0.join("projected-record-call-effect.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/projected_record_call_effect.th",
        "--level",
        "l3",
        "--export",
        "projected_call_pipeline",
        "--export",
        "projected_call_new",
        "--export",
        "projected_call_left_value",
        "--export",
        "projected_call_left_guard",
        "--export",
        "projected_call_right_value",
        "--export",
        "projected_call_right_guard",
        "--export",
        "projected_call_tag",
        "--crate-name",
        "projected_record_call_effect",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(
        source.contains("let written: u64 = projected_call_set(&mut outer.pair.left, value);")
            && source.contains("projected_call_copy(&mut outer.pair.right, &outer.pair.left)"),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("projected-record-call-effect-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use projected_record_call_effect::{
    projected_call_left_guard, projected_call_left_value, projected_call_new,
    projected_call_pipeline, projected_call_right_guard, projected_call_right_value,
    projected_call_tag,
};

fn main() {
    let mut outer = projected_call_new(3, 4, 5, 6, 7);
    assert_eq!(projected_call_pipeline(&mut outer, 41), 41);
    assert_eq!(projected_call_left_value(&outer), 41);
    assert_eq!(projected_call_left_guard(&outer), 4);
    assert_eq!(projected_call_right_value(&outer), 41);
    assert_eq!(projected_call_right_guard(&outer), 6);
    assert_eq!(projected_call_tag(&outer), 7);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("projected-record-call-effect-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "projected_record_call_effect={}",
            bundle
                .join("artifact/libprojected_record_call_effect.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert_eq!(receipt["binding"]["target"], "kernel");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "strict projected-record call lifecycle admitted a non-faithful row: {tv}"
    );
    assert!(rows.iter().any(|row| {
        row["phase"] == "body"
            && row["label"] == "projected_call_pipeline"
            && row["verdict"] == "faithful"
    }));
    assert!(!rows
        .iter()
        .any(|row| { row["phase"] == "exec" && row["label"] == "projected_call_pipeline.let#1" }));

    let tampered = temp
        .0
        .join("projected-record-call-effect-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen(
        "projected_call_copy(&mut outer.pair.right, &outer.pair.left)",
        "projected_call_copy(&mut outer.pair.right, &outer.pair.right)",
        1,
    );
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound projected-record borrow must invalidate the receipt"
    );
}

#[test]
fn projected_indexed_call_effect_is_strict_l3_replayed_and_executed() {
    let temp = TempDir::new("projected-indexed-call-effect");
    let bundle = temp.0.join("projected-indexed-call-effect.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/projected_indexed_call_effect.th",
        "--level",
        "l3",
        "--export",
        "projected_indexed_pipeline",
        "--export",
        "projected_indexed_left_zero",
        "--export",
        "projected_indexed_left_one",
        "--export",
        "projected_indexed_left_guard",
        "--export",
        "projected_indexed_right_zero",
        "--export",
        "projected_indexed_right_one",
        "--export",
        "projected_indexed_right_guard",
        "--export",
        "projected_indexed_tag",
        "--crate-name",
        "projected_indexed_call_effect",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(
        source.contains("projected_indexed_write(&mut outer.left.slots, value)")
            && source.contains("projected_indexed_copy(&mut outer.right.slots, &outer.left.slots)"),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("projected-indexed-call-effect-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use projected_indexed_call_effect::{
    projected_indexed_left_guard, projected_indexed_left_one,
    projected_indexed_left_zero, projected_indexed_pipeline,
    projected_indexed_right_guard, projected_indexed_right_one,
    projected_indexed_right_zero, projected_indexed_tag,
    ProjectedIndexedBank, ProjectedIndexedOuter,
};

fn main() {
    let mut outer = ProjectedIndexedOuter {
        left: ProjectedIndexedBank { slots: [3, 4], guard: 5 },
        right: ProjectedIndexedBank { slots: [6, 7], guard: 8 },
        tag: 9,
    };
    assert_eq!(projected_indexed_pipeline(&mut outer, 41), 41);
    assert_eq!(projected_indexed_left_zero(&outer), 41);
    assert_eq!(projected_indexed_left_one(&outer), 4);
    assert_eq!(projected_indexed_left_guard(&outer), 5);
    assert_eq!(projected_indexed_right_zero(&outer), 41);
    assert_eq!(projected_indexed_right_one(&outer), 7);
    assert_eq!(projected_indexed_right_guard(&outer), 8);
    assert_eq!(projected_indexed_tag(&outer), 9);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("projected-indexed-call-effect-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "projected_indexed_call_effect={}",
            bundle
                .join("artifact/libprojected_indexed_call_effect.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert_eq!(receipt["binding"]["target"], "kernel");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 51, "{tv}");
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "strict projected-indexed call lifecycle admitted a non-faithful row: {tv}"
    );
    assert!(rows.iter().any(|row| {
        row["phase"] == "body"
            && row["label"] == "projected_indexed_pipeline"
            && row["verdict"] == "faithful"
    }));
    for effectful_let in [
        "projected_indexed_pipeline.let#1",
        "projected_indexed_pipeline.let#2",
    ] {
        assert!(!rows
            .iter()
            .any(|row| row["phase"] == "exec" && row["label"] == effectful_let));
    }

    let tampered = temp
        .0
        .join("projected-indexed-call-effect-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen(
        "&mut outer.right.slots,\n    &outer.left.slots",
        "&mut outer.right.slots,\n    &outer.right.slots",
        1,
    );
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound projected indexed borrow must invalidate the receipt"
    );
}

#[test]
fn record_after_indexed_call_effect_is_strict_l3_replayed_and_executed() {
    let temp = TempDir::new("record-after-indexed-call-effect");
    let bundle = temp.0.join("record-after-indexed-call-effect.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/record_after_indexed_call_effect.th",
        "--level",
        "l3",
        "--export",
        "record_after_indexed_pipeline",
        "--export",
        "record_after_indexed_left_zero",
        "--export",
        "record_after_indexed_left_one",
        "--export",
        "record_after_indexed_left_guard",
        "--export",
        "record_after_indexed_right_zero",
        "--export",
        "record_after_indexed_right_one",
        "--export",
        "record_after_indexed_right_guard",
        "--export",
        "record_after_indexed_tag",
        "--crate-name",
        "record_after_indexed_call_effect",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(
        source.contains("record_after_indexed_write(&mut outer.left.slots, value)")
            && source.contains("record_after_indexed_advance(&mut outer.left, next_guard)",)
            && source.contains("record_after_indexed_copy(&mut outer.right, &outer.left)"),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("record-after-indexed-call-effect-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use record_after_indexed_call_effect::{
    record_after_indexed_left_guard, record_after_indexed_left_one,
    record_after_indexed_left_zero, record_after_indexed_pipeline,
    record_after_indexed_right_guard, record_after_indexed_right_one,
    record_after_indexed_right_zero, record_after_indexed_tag,
    RecordAfterIndexedBank, RecordAfterIndexedOuter,
};

fn main() {
    let mut outer = RecordAfterIndexedOuter {
        left: RecordAfterIndexedBank { slots: [3, 4], guard: 5 },
        right: RecordAfterIndexedBank { slots: [6, 7], guard: 8 },
        tag: 9,
    };
    assert_eq!(record_after_indexed_pipeline(&mut outer, 41, 55), 41);
    assert_eq!(record_after_indexed_left_zero(&outer), 41);
    assert_eq!(record_after_indexed_left_one(&outer), 41);
    assert_eq!(record_after_indexed_left_guard(&outer), 55);
    assert_eq!(record_after_indexed_right_zero(&outer), 41);
    assert_eq!(record_after_indexed_right_one(&outer), 7);
    assert_eq!(record_after_indexed_right_guard(&outer), 8);
    assert_eq!(record_after_indexed_tag(&outer), 9);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("record-after-indexed-call-effect-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "record_after_indexed_call_effect={}",
            bundle
                .join("artifact/librecord_after_indexed_call_effect.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert_eq!(receipt["binding"]["target"], "kernel");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 59, "{tv}");
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "record-after-indexed lifecycle admitted a non-faithful row: {tv}"
    );
    assert!(rows.iter().any(|row| {
        row["phase"] == "body"
            && row["label"] == "record_after_indexed_pipeline"
            && row["verdict"] == "faithful"
    }));
    for effectful_let in [
        "record_after_indexed_pipeline.let#1",
        "record_after_indexed_pipeline.let#2",
        "record_after_indexed_pipeline.let#3",
    ] {
        assert!(!rows
            .iter()
            .any(|row| row["phase"] == "exec" && row["label"] == effectful_let));
    }

    let tampered = temp
        .0
        .join("record-after-indexed-call-effect-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen(
        "&mut outer.left,\n    next_guard",
        "&mut outer.right,\n    next_guard",
        1,
    );
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the record actual after indexed state must invalidate the receipt"
    );
}

#[test]
fn mutable_indexed_call_effect_is_strict_l3_replayed_and_executed() {
    let temp = TempDir::new("mutable-indexed-call-effect");
    let bundle = temp.0.join("mutable-indexed-call-effect.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/mutable_indexed_call_effect.th",
        "--level",
        "l3",
        "--export",
        "indexed_call_pipeline",
        "--crate-name",
        "mutable_indexed_call_effect",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(
        source.contains("data: &mut [u64; INDEXED_CALL_SLOTS]")
            && source.contains("let next: u64 = value;")
            && source.contains("let observed: u64 = indexed_call_write_zero(data, next);"),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("mutable-indexed-call-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use mutable_indexed_call_effect::indexed_call_pipeline;

fn main() {
    let mut data = [3_u64, 77_u64];
    assert_eq!(indexed_call_pipeline(&mut data, 41), 41);
    assert_eq!(data, [41, 77]);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("mutable-indexed-call-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "mutable_indexed_call_effect={}",
            bundle
                .join("artifact/libmutable_indexed_call_effect.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "strict indexed-call lifecycle admitted a non-faithful row: {tv}"
    );
    assert!(rows.iter().any(|row| {
        row["phase"] == "body"
            && row["label"] == "indexed_call_pipeline"
            && row["verdict"] == "faithful"
    }));
    assert!(rows.iter().any(|row| {
        row["phase"] == "exec"
            && row["label"] == "indexed_call_pipeline.let#1"
            && row["verdict"] == "faithful"
    }));
    assert!(!rows
        .iter()
        .any(|row| { row["phase"] == "exec" && row["label"] == "indexed_call_pipeline.let#2" }));

    let tampered = temp.0.join("mutable-indexed-call-effect-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen(
        "indexed_call_write_zero(data, next)",
        "indexed_call_write_zero(data, next + 1)",
        1,
    );
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound indexed-call effect must invalidate the receipt"
    );
}

#[test]
fn mixed_indexed_call_effect_is_strict_l3_replayed_and_executed() {
    let temp = TempDir::new("mixed-indexed-call-effect");
    let bundle = temp.0.join("mixed-indexed-call-effect.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/mixed_indexed_call_effect.th",
        "--level",
        "l3",
        "--export",
        "mixed_indexed_pipeline",
        "--crate-name",
        "mixed_indexed_call_effect",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n"), "{source}");
    assert!(
        source.contains("left: &mut [u64; MIXED_INDEXED_SLOTS]")
            && source.contains("right: &mut [u64; MIXED_INDEXED_SLOTS]")
            && source.contains("right[1] = value;")
            && source.contains("mixed_indexed_copy(left, right)"),
        "{source}"
    );
    assert!(!source.contains("external_body"), "{source}");

    let consumer_source = temp.0.join("mixed-indexed-call-consumer.rs");
    fs::write(
        &consumer_source,
        r#"
use mixed_indexed_call_effect::mixed_indexed_pipeline;

fn main() {
    let mut left = [3_u64, 4_u64];
    let mut right = [5_u64, 6_u64];
    assert_eq!(mixed_indexed_pipeline(&mut left, &mut right, 41), 41);
    assert_eq!(left, [41, 4]);
    assert_eq!(right, [5, 41]);
}
"#,
    )
    .unwrap();
    let consumer = temp.0.join("mixed-indexed-call-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!(
            "mixed_indexed_call_effect={}",
            bundle
                .join("artifact/libmixed_indexed_call_effect.rlib")
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
    assert_success(&link);
    assert_success(&Command::new(&consumer).output().unwrap());

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    assert_eq!(receipt["binding"]["assurance"], "L3");
    assert!(receipt["binding"]["assurance_aggregate"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|member| member["achieved"] == "L3"));

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "strict mixed-indexed lifecycle admitted a non-faithful row: {tv}"
    );
    assert!(rows.iter().any(|row| {
        row["phase"] == "body"
            && row["label"] == "mixed_indexed_pipeline"
            && row["verdict"] == "faithful"
    }));
    assert!(!rows
        .iter()
        .any(|row| { row["phase"] == "exec" && row["label"] == "mixed_indexed_pipeline.let#1" }));

    let tampered = temp.0.join("mixed-indexed-call-effect-tampered.verified");
    copy_tree(&bundle, &tampered);
    let input = tampered.join("evidence/input.th");
    let original = fs::read_to_string(&input).unwrap();
    let changed = original.replacen(
        "mixed_indexed_copy(left, right)",
        "mixed_indexed_copy(left, left)",
        1,
    );
    assert_ne!(changed, original);
    fs::write(&input, changed).unwrap();
    assert!(
        !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
            .status
            .success(),
        "changing the bound mixed indexed borrow must invalidate the receipt"
    );
}

#[test]
fn aggregate_mutable_storage_is_exported_replayed_and_exactly_validated() {
    let temp = TempDir::new("aggregate-storage");
    let bundle = temp.0.join("aggregate-storage.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/aggregate_storage.th",
        "--level",
        "l3",
        "--export",
        "write_row",
        "--crate-name",
        "aggregate_storage",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains("data: &mut [[u64; 2]]"), "{source}");
    assert!(source.contains("data[at] = [value, value];"), "{source}");
    assert!(
        source.contains("final(data)@[at as int]@[0 as int] == value"),
        "{source}"
    );

    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    let export = &receipt["binding"]["exports"][0];
    assert_eq!(
        export["parameter_types"],
        serde_json::json!(["&mut [[u64;2]]", "usize", "u64"])
    );
    assert_eq!(
        export["ownership"],
        serde_json::json!(["exclusive_borrow", "by_value", "by_value"])
    );

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let rows = tv["rows"].as_array().unwrap();
    assert!(rows.iter().any(|row| {
        row["phase"] == "contract"
            && row["label"] == "write_row.ens#2"
            && row["verdict"] == "faithful"
    }));
    assert!(rows.iter().any(|row| {
        row["phase"] == "body" && row["label"] == "write_row" && row["verdict"] == "faithful"
    }));
    assert!(
        rows.iter().all(|row| row["verdict"] == "faithful"),
        "every mutable-storage contract/expression/body/wrapper row must be faithful: {tv}"
    );
}

#[test]
fn total_wrapper_returns_ok_or_precondition_without_calling_invalid_body() {
    let temp = TempDir::new("wrapper");
    let bundle = temp.0.join("wrapper.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/bounded_inc.th",
        "--level",
        "l3",
        "--export",
        "bounded_inc",
        "--crate-name",
        "bounded_guard_tv",
        "--out",
        &bundle_s,
    ]));
    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains("pub fn thermite_export_bounded_inc_v1"));
    assert!(source.contains("Err(ThermiteContractError::Precondition)"));
    let tv = fs::read_to_string(bundle.join("evidence/translation-validation.json")).unwrap();
    assert!(tv.contains("wrapper_guard"));

    let consumer = temp.0.join("consumer");
    let output = codegen_rustc(&bundle)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-build/wrapper_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!(
            "bounded_guard_tv={}",
            bundle.join("artifact/libbounded_guard_tv.rlib").display()
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
    assert_success(&output);
    assert_success(&Command::new(consumer).output().unwrap());
}

#[test]
fn only_the_declared_export_is_public_across_a_transitive_closure() {
    let temp = TempDir::new("visibility");
    let bundle = temp.0.join("visibility.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "conformance/verified-build/closure.th",
        "--level",
        "l3",
        "--export",
        "closure_root",
        "--crate-name",
        "closure_visibility",
        "--out",
        &bundle_s,
    ]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains("pub fn closure_root"));
    assert!(source.contains("\nfn helper"));
    assert!(!source.contains("pub fn helper"));
    assert!(!source.contains("unrelated"));

    let artifact = bundle.join("artifact/libclosure_visibility.rlib");
    let deps = bundle.join("artifact/deps");
    let consumer = temp.0.join("closure-consumer");
    let link = codegen_rustc(&bundle)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-build/closure_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!("closure_visibility={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(&consumer)
        .output()
        .unwrap();
    assert_success(&link);
    assert_success(&Command::new(consumer).output().unwrap());

    let forbidden = codegen_rustc(&bundle)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-build/private_helper_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!("closure_visibility={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(temp.0.join("must-not-link"))
        .output()
        .unwrap();
    assert!(!forbidden.status.success());
    assert!(String::from_utf8_lossy(&forbidden.stderr).contains("private"));
}

#[test]
fn kernel_bundle_final_links_into_a_separate_no_std_consumer() {
    let temp = TempDir::new("kernel");
    let bundle = temp.0.join("kernel.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge_with_incompatible_host_rustc(&[
        "build",
        "conformance/verified-build/identity.th",
        "--level",
        "l3",
        "--export",
        "identity",
        "--crate-name",
        "kernel_identity",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
    ]));
    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.starts_with("#![no_std]\n#![crate_type = \"rlib\"]"));
    // Kernel verification intentionally imports Forge's digest-bound no_std
    // vstd proof prelude. Specification code erases from the artifact; the
    // executable crate remains no_std and links into the freestanding consumer.
    assert_eq!(source.matches("use vstd::prelude::*;").count(), 1);
    assert!(!source.contains("extern crate std"));

    let consumer = temp.0.join("kernel-consumer");
    let output = codegen_rustc(&bundle)
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-build/kernel_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!(
            "kernel_identity={}",
            bundle.join("artifact/libkernel_identity.rlib").display()
        ))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            bundle.join("artifact/deps").display()
        ))
        .args(["-C", "panic=abort", "-C", "link-arg=-nostartfiles"])
        .arg("-o")
        .arg(&consumer)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(consumer.is_file());

    let incompatible = incompatible_rustc()
        .current_dir(root())
        .args([
            "--edition=2021",
            "conformance/verified-build/kernel_consumer.rs",
        ])
        .arg("--extern")
        .arg(format!(
            "kernel_identity={}",
            bundle.join("artifact/libkernel_identity.rlib").display()
        ))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            bundle.join("artifact/deps").display()
        ))
        .args(["-C", "panic=abort", "-C", "link-arg=-nostartfiles"])
        .arg("-o")
        .arg(temp.0.join("incompatible-kernel-consumer"))
        .output()
        .unwrap();
    assert!(!incompatible.status.success());
    let incompatible_stderr = String::from_utf8_lossy(&incompatible.stderr);
    assert!(
        incompatible_stderr.contains("incompatible version of rustc")
            && incompatible_stderr.contains("compiled by rustc 1.95.0"),
        "{incompatible_stderr}"
    );

    assert_success(&forge_with_incompatible_host_rustc(&[
        "verify-build",
        &bundle_s,
        "--replay",
    ]));
}

#[test]
fn package_build_binds_and_replays_the_complete_source_identified_closure() {
    let temp = TempDir::new("package");
    let package_root = temp.0.join("package");
    fs::create_dir(&package_root).unwrap();
    fs::create_dir(package_root.join("src")).unwrap();
    fs::write(
        package_root.join("src/base.th"),
        b"fn package_base(x: u64) -> u64 req true ens result == x fx pure { x }\n",
    )
    .unwrap();
    fs::write(
        package_root.join("src/api.th"),
        b"fn package_api(x: u64) -> u64 req true ens result == x fx pure { package_base(x) }\n",
    )
    .unwrap();
    let manifest = r#"{
  "schema": "thermite.package.v1",
  "name": "package_primitives",
  "roots": [
    "api"
  ],
  "modules": [
    {
      "name": "api",
      "path": "src/api.th",
      "imports": [
        "base"
      ]
    },
    {
      "name": "base",
      "path": "src/base.th",
      "imports": []
    }
  ]
}
"#;
    let manifest_path = package_root.join("primitives.thpkg.json");
    fs::write(&manifest_path, manifest).unwrap();
    let bundle = temp.0.join("package.verified");
    let manifest_s = manifest_path.to_string_lossy().to_string();
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        &manifest_s,
        "--level",
        "l3",
        "--export",
        "package_api",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
    ]));

    for relative in [
        "evidence/thermite-package/manifest.json",
        "evidence/thermite-package/source-map.json",
        "evidence/thermite-package/source/src/base.th",
        "evidence/thermite-package/source/src/api.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }
    assert!(bundle.join("artifact/libpackage_primitives.rlib").is_file());

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["package"]["name"], "package_primitives");
    assert_eq!(plan["package"]["roots"], serde_json::json!(["api"]));
    let nodes = plan["closure_nodes"].as_array().unwrap();
    let base = nodes
        .iter()
        .find(|node| node["name"] == "package_base")
        .unwrap();
    let api = nodes
        .iter()
        .find(|node| node["name"] == "package_api")
        .unwrap();
    assert_eq!(base["source_module"], "base");
    assert_eq!(base["source_path"], "src/base.th");
    assert_eq!(api["source_module"], "api");
    assert_eq!(api["source_path"], "src/api.th");
    assert_eq!(base["source_start"], 0);
    assert_eq!(api["source_start"], 0);

    assert_success(&forge(&["verify-build", &bundle_s, "--replay"]));

    for (index, relative) in [
        "evidence/thermite-package/manifest.json",
        "evidence/thermite-package/source-map.json",
        "evidence/thermite-package/source/src/base.th",
        "evidence/thermite-package/source/src/api.th",
    ]
    .iter()
    .enumerate()
    {
        let tampered = temp.0.join(format!("package-tampered-{index}.verified"));
        copy_tree(&bundle, &tampered);
        let path = tampered.join(relative);
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b' ');
        fs::write(path, bytes).unwrap();
        assert!(
            !forge(&["verify-build", tampered.to_string_lossy().as_ref()])
                .status
                .success(),
            "tampering `{relative}` was accepted"
        );
    }
}

#[test]
fn atomic_primitive_package_keeps_every_in_language_item_at_l3() {
    let temp = TempDir::new("atomic-primitives");
    let manifest = root().join("stdlib/kernel-primitives/atomics.thpkg.json");
    let manifest_s = manifest.to_string_lossy().to_string();

    let machine_source =
        fs::read_to_string(root().join("stdlib/kernel-primitives/src/machine.th")).unwrap();
    let machine_program = thermite_syntax::parse(&machine_source);
    assert!(
        machine_program.is_clean(),
        "atomic machine ABI must parse clean: {:?}",
        machine_program.errors
    );
    let machine_functions = machine_program
        .program
        .items
        .iter()
        .filter_map(|item| match item {
            thermite_syntax::Item::Fn(function) => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(machine_functions.len(), 50);
    for function in machine_functions {
        assert!(function.name.starts_with("atomic_machine_"));
        assert!(function.boundary.is_some());
        assert!(function.body.is_none());
        assert!(function.slag.is_none());
        assert_ne!(function.contract.ens[0].text, "true");

        let req = function.contract.req.text.as_str();
        let ens = function.contract.ens[0].text.as_str();
        if function.name.ends_with("_init") {
            assert_eq!(req, "true");
            assert!(
                ens.contains("result.1 == authority")
                    && ens.contains("result.2 == storage_slot")
                    && ens.contains("result.3 == generation"),
                "atomic initializer lost its persistent identity echo: {}",
                function.name
            );
        } else if function.name.ends_with("_load") {
            assert_eq!(req, "atomic_load_code_legal_spec(order)");
            assert_eq!(ens, "result.1 == handle");
        } else if function.name.ends_with("_store") {
            assert_eq!(req, "atomic_store_code_legal_spec(order)");
            assert_eq!(ens, "result.0 == value && result.1 == handle");
        } else if function.name.contains("compare_exchange") {
            assert_eq!(req, "atomic_cas_code_legal_spec(success, failure)");
            assert!(ens.contains("result.2 == handle"));
        } else if function.name.ends_with("_fence") {
            assert_eq!(req, "atomic_fence_code_legal_spec(order)");
            assert_eq!(ens, "result");
        } else {
            assert_eq!(req, "atomic_rmw_code_legal_spec(order)");
            assert_eq!(ens, "result.1 == handle");
        }
    }

    let model = forge(&[
        "check",
        "stdlib/kernel-primitives/src/model.th",
        "--level",
        "l3",
        "--json",
    ]);
    assert_success(&model);
    let model_rows: serde_json::Value = serde_json::from_slice(&model.stdout).unwrap();
    let model_rows = model_rows.as_array().unwrap();
    assert_eq!(model_rows.len(), 60);
    assert!(
        model_rows
            .iter()
            .all(|row| row["level"] == "L3" && row["boundary"] == false),
        "an in-language atomic model item fell below L3: {model_rows:?}"
    );

    let projection = temp.0.join("atomic-primitives-projection.th");
    let mut projection_source =
        fs::read_to_string(root().join("stdlib/kernel-primitives/src/model.th")).unwrap();
    for source in [
        "stdlib/kernel-primitives/storage/static_storage.th",
        "stdlib/kernel-primitives/src/init.th",
        "stdlib/kernel-primitives/src/machine.th",
        "stdlib/kernel-primitives/src/api.th",
        "stdlib/kernel-primitives/src/atomic_storage.th",
    ] {
        projection_source.push('\n');
        projection_source.push_str(&fs::read_to_string(root().join(source)).unwrap());
    }
    fs::write(&projection, &projection_source).unwrap();
    let projection_s = projection.to_string_lossy().to_string();
    let checked = forge(&["check", &projection_s, "--level", "l3", "--json"]);
    assert_success(&checked);
    let rows: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(
        rows.iter().filter(|row| row["boundary"] == true).count(),
        52
    );
    for row in rows {
        if row["boundary"] == true {
            assert_eq!(
                row["level"], "L1",
                "a bodyless atomic machine declaration changed assurance class: {row}"
            );
        } else {
            assert_eq!(
                row["level"], "L3",
                "an in-language atomic primitive fell below L3: {row}"
            );
        }
    }

    let machine_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| {
            row["boundary"] == true
                && row["item"]
                    .as_str()
                    .is_some_and(|name| name.starts_with("atomic_machine_"))
        })
        .collect();
    assert_eq!(
        machine_rows.len(),
        50,
        "the frozen atomic machine ABI drifted"
    );
    for machine in machine_rows {
        let machine_name = machine["item"].as_str().unwrap();
        let app_name = machine_name.replacen("atomic_machine_", "atomic_", 1);
        let app = rows
            .iter()
            .find(|row| row["item"] == app_name)
            .unwrap_or_else(|| panic!("missing bodyful atomic app primitive `{app_name}`"));
        assert_eq!(app["boundary"], false, "`{app_name}` remained a boundary");
        assert_eq!(app["level"], "L3", "`{app_name}` fell below L3: {app}");
    }

    for name in [
        "atomic_identity_eq",
        "atomic_identity_matches_values",
        "atomic_init_capacity_legal",
        "atomic_bool_slot_from_region",
        "atomic_u32_slot_from_region",
        "atomic_u64_slot_from_region",
        "atomic_usize_slot_from_region",
        "atomic_bool_region_init",
        "atomic_u32_region_init",
        "atomic_u64_region_init",
        "atomic_usize_region_init",
        "atomic_bool_storage_after_claim",
        "atomic_u32_storage_after_claim",
        "atomic_u64_storage_after_claim",
        "atomic_usize_storage_after_claim",
        "atomic_u64_storage_lifecycle_probe",
    ] {
        let row = rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing atomic-storage certificate `{name}`"));
        assert_eq!(row["level"], "L3", "`{name}` fell below L3: {row}");
        assert_ne!(
            row["contract_quality"]["mutants_killed"], "0/0",
            "`{name}` lacks executable contract teeth",
        );
    }

    let mut duplicate_source = projection_source.clone();
    duplicate_source.push_str(
        r#"
fn duplicate_atomic_u64_slot_rejected(slot: AtomicU64Slot) -> bool
  req true
  ens result
  fx platform(atomic)
{
  let first: AtomicU64 = atomic_u64_init(slot, 0);
  let second: AtomicU64 = atomic_u64_init(slot, 1);
  atomic_identity_eq(&first.identity, &second.identity)
}
"#,
    );
    let duplicate = temp.0.join("duplicate-atomic-slot.th");
    fs::write(&duplicate, duplicate_source).unwrap();
    let duplicate_s = duplicate.to_string_lossy().to_string();
    let duplicate_check = forge(&["check", &duplicate_s, "--level", "l3", "--json"]);
    assert!(
        !duplicate_check.status.success(),
        "one opaque atomic-init slot initialized two cells",
    );
    let duplicate_rows: serde_json::Value =
        serde_json::from_slice(&duplicate_check.stdout).unwrap();
    let duplicate_row = duplicate_rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["item"] == "duplicate_atomic_u64_slot_rejected")
        .unwrap();
    assert_ne!(duplicate_row["level"], "L3");
    let duplicate_diagnostic = duplicate_row["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|obligation| obligation["diagnostic"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        duplicate_diagnostic.contains("use of moved value: `slot`"),
        "duplicate slot failed for the wrong reason: {duplicate_row}",
    );

    let opaque_attack = temp.0.join("opaque-atomic-slot");
    fs::create_dir(&opaque_attack).unwrap();
    for (source, destination) in [
        ("stdlib/kernel-primitives/src/model.th", "model.th"),
        (
            "stdlib/kernel-primitives/storage/static_storage.th",
            "static_storage.th",
        ),
        ("stdlib/kernel-primitives/src/init.th", "init.th"),
    ] {
        fs::copy(root().join(source), opaque_attack.join(destination)).unwrap();
    }
    fs::write(
        opaque_attack.join("attack.th"),
        r#"fn forge_atomic_u64_slot(
  authority: usize,
  storage_slot: usize,
  generation: u64,
  capacity: usize,
) -> AtomicU64Slot
  req true
  ens atomic_u64_slot_matches_values_spec(
    &result,
    authority,
    storage_slot,
    generation,
    capacity,
  )
  fx pure
{
  AtomicU64Slot {
    atomic_slot_identity: AtomicIdentity {
      authority: authority,
      slot: storage_slot,
      generation: generation,
    },
    atomic_slot_bytes: capacity,
  }
}
"#,
    )
    .unwrap();
    fs::write(
        opaque_attack.join("attack.thpkg.json"),
        r#"{
  "schema": "thermite.package.v1",
  "name": "opaque_atomic_slot_attack",
  "roots": [
    "attack"
  ],
  "modules": [
    {
      "name": "attack",
      "path": "attack.th",
      "imports": [
        "init",
        "model"
      ]
    },
    {
      "name": "init",
      "path": "init.th",
      "imports": [
        "model",
        "static_storage"
      ]
    },
    {
      "name": "model",
      "path": "model.th",
      "imports": []
    },
    {
      "name": "static_storage",
      "path": "static_storage.th",
      "imports": []
    }
  ]
}
"#,
    )
    .unwrap();
    let attack_manifest = opaque_attack.join("attack.thpkg.json");
    let attack_manifest_s = attack_manifest.to_string_lossy().to_string();
    let attack_bundle = opaque_attack.join("attack.verified");
    let attack_bundle_s = attack_bundle.to_string_lossy().to_string();
    let attack = forge(&[
        "build",
        &attack_manifest_s,
        "--level",
        "l3",
        "--export",
        "forge_atomic_u64_slot",
        "--target",
        "kernel",
        "--out",
        &attack_bundle_s,
        "--json",
    ]);
    assert!(
        !attack.status.success(),
        "a foreign package module constructed an opaque atomic-init slot",
    );
    let attack_diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&attack.stdout),
        String::from_utf8_lossy(&attack.stderr),
    );
    assert!(
        attack_diagnostic.contains("constructs `#[opaque]` type `AtomicU64Slot`")
            && attack_diagnostic.contains("declared in module `init`"),
        "unexpected opaque-slot diagnostic: {attack_diagnostic}",
    );

    for (name, export, target) in [
        ("ordering", "atomic_ordering_matrix_probe", "kernel"),
        ("history", "atomic_history_model_probe", "std"),
        ("storage", "atomic_storage_capacity_probe", "kernel"),
    ] {
        let bundle = temp.0.join(format!("{name}.verified"));
        let bundle_s = bundle.to_string_lossy().to_string();
        assert_success(&forge(&[
            "build",
            &manifest_s,
            "--level",
            "l3",
            "--export",
            export,
            "--target",
            target,
            "--out",
            &bundle_s,
            "--json",
        ]));
        assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

        for relative in [
            "evidence/thermite-package/manifest.json",
            "evidence/thermite-package/source-map.json",
            "evidence/thermite-package/source/src/model.th",
            "evidence/thermite-package/source/src/machine.th",
            "evidence/thermite-package/source/src/api.th",
            "evidence/thermite-package/source/src/init.th",
            "evidence/thermite-package/source/src/atomic_storage.th",
            "evidence/thermite-package/source/storage/static_storage.th",
        ] {
            assert!(
                bundle.join(relative).is_file(),
                "{name} bundle omitted `{relative}`"
            );
        }
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
        assert_eq!(receipt["binding"]["exports"][0]["thermite_name"], export);
        assert_eq!(receipt["binding"]["assurance"], "L3");
        assert!(receipt["binding"]["assurance_aggregate"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .all(|member| member["achieved"] == "L3"));
        let plan: serde_json::Value =
            serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
                .unwrap();
        assert_eq!(plan["package"]["name"], "thermite_atomic_primitives");

        let tv: serde_json::Value = serde_json::from_slice(
            &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
        )
        .unwrap();
        let rows = tv["rows"].as_array().unwrap();
        assert!(
            rows.iter().all(|row| row["verdict"] == "faithful"),
            "{name} bundle contains non-faithful translation validation rows: {tv}"
        );

        if name == "storage" {
            let consumer_source = temp.0.join("atomic-storage-consumer.rs");
            fs::write(
                &consumer_source,
                r#"fn main() {
    use thermite_atomic_primitives::atomic_storage_capacity_probe;
    assert!(!atomic_storage_capacity_probe(0, 0));
    assert!(atomic_storage_capacity_probe(0, 1));
    assert!(!atomic_storage_capacity_probe(1, 3));
    assert!(atomic_storage_capacity_probe(1, 4));
    assert!(!atomic_storage_capacity_probe(2, 7));
    assert!(atomic_storage_capacity_probe(2, 8));
    assert!(!atomic_storage_capacity_probe(3, 7));
    assert!(atomic_storage_capacity_probe(3, 8));
    assert!(!atomic_storage_capacity_probe(4, 64));
}
"#,
            )
            .unwrap();
            let consumer = temp.0.join("atomic-storage-consumer");
            let compiled = codegen_rustc(&bundle)
                .current_dir(root())
                .arg("--edition=2021")
                .arg(&consumer_source)
                .arg("--extern")
                .arg(format!(
                    "thermite_atomic_primitives={}",
                    bundle
                        .join("artifact/libthermite_atomic_primitives.rlib")
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
            assert_success(&compiled);
            assert_success(&Command::new(&consumer).output().unwrap());

            let bound_init = bundle.join("evidence/thermite-package/source/src/init.th");
            let original = fs::read_to_string(&bound_init).unwrap();
            let weakened =
                original.replacen("#[opaque] struct AtomicU64Slot", "struct AtomicU64Slot", 1);
            assert_ne!(weakened, original);
            fs::write(&bound_init, weakened).unwrap();
            assert!(
                !forge(&["verify-build", &bundle_s, "--replay", "--json"])
                    .status
                    .success(),
                "removing the receipt-bound atomic-slot barrier replayed",
            );
        }
    }
}

#[test]
fn every_strict_refusal_publishes_nothing() {
    for (file, export, target, expected) in [
        ("bad_body.th", "bad_identity", None, "certificates"),
        ("boundary.th", "boundary_root", None, "boundary"),
        ("unresolved.th", "unresolved_root", None, "unresolved"),
        ("slag.th", "slag_root", None, "slag"),
        ("diverge.th", "diverge_root", None, "diverge"),
        (
            "non_executable_req.th",
            "guarded_by_spec",
            None,
            "non-executable",
        ),
        ("tv_skipped.th", "tv_skipped", None, "skipped"),
        (
            "kernel_ambient.th",
            "reads_clock_without_using_it",
            Some("kernel"),
            "ambient",
        ),
        (
            "transitive_boundary.th",
            "transitive_boundary_root",
            None,
            "transitive_boundary_root -> boundary_middle -> foreign_identity",
        ),
        (
            "transitive_slag.th",
            "transitive_slag_root",
            None,
            "transitive_slag_root -> slag_middle -> transitive_vendored",
        ),
        (
            "transitive_unresolved.th",
            "transitive_unresolved_root",
            None,
            "transitive_unresolved_root -> unresolved_middle",
        ),
        (
            "transitive_diverge.th",
            "transitive_diverge_root",
            None,
            "transitive_diverge_root -> diverge_middle -> transitive_diverging",
        ),
    ] {
        let temp = TempDir::new(file);
        let bundle = temp.0.join("must-not-exist.verified");
        let source = format!("conformance/verified-build/{file}");
        let mut args = vec![
            "build".to_string(),
            source,
            "--level".to_string(),
            "l3".to_string(),
            "--export".to_string(),
            export.to_string(),
            "--out".to_string(),
            bundle.display().to_string(),
            "--json".to_string(),
        ];
        if let Some(target) = target {
            args.extend(["--target".to_string(), target.to_string()]);
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = forge(&refs);
        assert_eq!(output.status.code(), Some(1), "{file}");
        assert!(!bundle.exists(), "{file} published a partial bundle");
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains(expected), "{file}: {text}");
    }
}

#[test]
fn every_bad_body_mutation_is_source_located_and_publishes_nothing() {
    for (class, file, export) in [
        ("operator", "bad_operator.th", "bad_operator"),
        ("branch", "bad_branch.th", "bad_branch"),
        ("return", "bad_body.th", "bad_identity"),
        ("loop update", "bad_loop_update.th", "bad_loop_update"),
        ("call", "bad_call.th", "bad_call"),
    ] {
        let temp = TempDir::new(class);
        let bundle = temp.0.join("must-not-exist.verified");
        let source = format!("conformance/verified-build/{file}");
        let output = forge(&[
            "build",
            &source,
            "--level",
            "l3",
            "--export",
            export,
            "--out",
            bundle.to_string_lossy().as_ref(),
            "--json",
        ]);
        assert_eq!(output.status.code(), Some(1), "{class}");
        assert!(!bundle.exists(), "{class} mutation published a bundle");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("certificates"), "{class}: {stdout}");
        assert!(stdout.contains("Thermite bytes"), "{class}: {stdout}");
        assert!(stdout.contains("error:"), "{class}: {stdout}");
    }
}

#[test]
fn every_injected_commitment_failure_is_atomic() {
    let temp = TempDir::new("faults");
    for (fault, file, export) in [
        ("after-plan-source-mutation", "identity.th", "identity"),
        ("after-plan-body-mutation", "identity.th", "identity"),
        ("after-plan-helper-mutation", "closure.th", "closure_root"),
        (
            "after-plan-wrapper-mutation",
            "bounded_inc.th",
            "bounded_inc",
        ),
        ("before-verus", "identity.th", "identity"),
        ("after-verus", "identity.th", "identity"),
        ("after-codegen", "identity.th", "identity"),
        ("after-artifact-hash", "identity.th", "identity"),
        ("after-plan-hash", "identity.th", "identity"),
        ("after-evidence-hash", "identity.th", "identity"),
        ("after-toolchain-hash", "identity.th", "identity"),
        ("after-receipt-staging", "identity.th", "identity"),
    ] {
        let bundle = temp.0.join(format!("{fault}.verified"));
        let source = format!("conformance/verified-build/{file}");
        let output = Command::new(env!("CARGO_BIN_EXE_forge"))
            .current_dir(root())
            .env("THERMITE_L3_TEST_FAULT", fault)
            .args([
                "build",
                &source,
                "--level",
                "l3",
                "--export",
                export,
                "--crate-name",
                "fault_identity",
                "--out",
                bundle.to_string_lossy().as_ref(),
            ])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "fault `{fault}` unexpectedly succeeded"
        );
        assert!(!bundle.exists(), "fault `{fault}` published a bundle");

        let stage_prefix = format!(".{fault}.verified.stage.");
        let leaked = fs::read_dir(&temp.0)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&stage_prefix)
            });
        assert!(!leaked, "fault `{fault}` leaked a staging tree");
    }
}

#[test]
fn every_tv_phase_and_nonpass_class_blocks_publication() {
    let temp = TempDir::new("tv-matrix");
    for (phase, file, export) in [
        ("contract", "identity.th", "identity"),
        ("exec", "identity.th", "identity"),
        ("body", "identity.th", "identity"),
        ("loop", "loop_count.th", "count_to"),
    ] {
        for verdict in ["divergent", "unsupported", "skipped", "unverifiable"] {
            let fault = format!("tv-{phase}-{verdict}");
            let bundle = temp.0.join(format!("{phase}-{verdict}.verified"));
            let source = format!("conformance/verified-build/{file}");
            let output = Command::new(env!("CARGO_BIN_EXE_forge"))
                .current_dir(root())
                .env("THERMITE_L3_TEST_FAULT", &fault)
                .args([
                    "build",
                    &source,
                    "--level",
                    "l3",
                    "--export",
                    export,
                    "--crate-name",
                    "tv_matrix",
                    "--out",
                    bundle.to_string_lossy().as_ref(),
                    "--json",
                ])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{fault}");
            assert!(!bundle.exists(), "{fault} published a bundle");
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains(phase), "{fault}: {stdout}");
            assert!(stdout.contains(verdict), "{fault}: {stdout}");
        }
    }
}

#[test]
fn every_non_l3_certificate_class_blocks_publication() {
    let temp = TempDir::new("certificate-matrix");
    for (fault, expected) in [
        ("certificate-l1", "L1"),
        ("certificate-l2", "L2"),
        ("certificate-timeout", "degraded"),
        ("certificate-counterexample", "L0"),
        ("certificate-rejected", "rejected"),
        ("certificate-failed-obligation", "failed obligation"),
        ("certificate-missing", "missing"),
    ] {
        let bundle = temp.0.join(format!("{fault}.verified"));
        let output = Command::new(env!("CARGO_BIN_EXE_forge"))
            .current_dir(root())
            .env("THERMITE_L3_TEST_FAULT", fault)
            .args([
                "build",
                "conformance/verified-build/identity.th",
                "--level",
                "l3",
                "--export",
                "identity",
                "--crate-name",
                "certificate_matrix",
                "--out",
                bundle.to_string_lossy().as_ref(),
                "--json",
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{fault}");
        assert!(!bundle.exists(), "{fault} published a bundle");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(expected), "{fault}: {stdout}");
    }
}
