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
        "--crate-name",
        "fixed_array_read",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    let source = fs::read_to_string(bundle.join("evidence/source.verus.rs")).unwrap();
    assert!(source.contains("pub const SLOTS: usize = 4;"), "{source}");
    assert!(
        source.contains("let slots: [u64; SLOTS] = [7; SLOTS];"),
        "{source}"
    );
    assert!(source.contains("slots[at]"), "{source}");
    assert!(
        source.contains("pub trait __thermite_FixedArrayEq"),
        "{source}"
    );
    assert!(source.contains("__thermite_fixed_array_eq"), "{source}");

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
    assert!(!source.contains("use vstd::"));

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
