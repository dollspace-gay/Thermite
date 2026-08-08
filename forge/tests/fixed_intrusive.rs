//! Primitive-only acceptance for allocation-free intrusive-list metadata.
//!
//! Link storage, fail-closed push/pop/unlink transitions, and policy-free list
//! mechanics are authored in Thermite. Every bodyful item must certify at L3;
//! this package has no platform boundary and no parallel Rust implementation.

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
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_fixed_intrusive_{name}_{}_{}",
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

fn mutation_total(rows: &[serde_json::Value]) -> (u64, u64) {
    rows.iter().fold((0, 0), |(killed, total), row| {
        let score = row["contract_quality"]["mutants_killed"].as_str().unwrap();
        let (row_killed, row_total) = score.split_once('/').unwrap();
        (
            killed + row_killed.parse::<u64>().unwrap(),
            total + row_total.parse::<u64>().unwrap(),
        )
    })
}

#[test]
fn fixed_intrusive_is_l3_fail_closed_receipt_bound_and_executable() {
    let source = "stdlib/kernel-primitives/collections/intrusive.th";
    let temp = TempDir::new("acceptance");

    let checked = forge(&["check", source, "--level", "l3", "--json"]);
    assert_success(&checked);
    let rows: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 39);
    assert!(
        rows.iter()
            .all(|row| row["level"] == "L3" && row["boundary"] == false),
        "an intrusive-metadata item fell below boundary-free L3: {rows:?}",
    );
    // `fixed_intrusive_empty` returns `FixedIntrusiveList64`, whose array fields
    // became zero-able when `zero_value_for` in `mutation.rs` learned to zero a
    // fixed array. It gained one early-return zero mutant, which dies on the
    // declared head, tail, and length. `FixedIntrusiveEndpoints64` holds only
    // scalars, so it already had a zero and gained nothing.
    assert_eq!(mutation_total(rows), (272, 282));
    for name in [
        "fixed_intrusive_empty",
        "fixed_intrusive_link_reason",
        "fixed_intrusive_pop_reason",
        "fixed_intrusive_link_empty",
        "fixed_intrusive_link_nonempty",
        "fixed_intrusive_push_back",
        "fixed_intrusive_pop_last",
        "fixed_intrusive_pop_more",
        "fixed_intrusive_pop_front",
        "fixed_intrusive_unlink_reason",
        "fixed_intrusive_unlink_live",
        "fixed_intrusive_unlink",
        "fixed_intrusive_fifo_probe",
        "fixed_intrusive_unlink_middle_probe",
        "fixed_intrusive_endpoints_probe",
        "fixed_intrusive_duplicate_probe",
    ] {
        let row = rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing intrusive certificate for `{name}`"));
        assert_ne!(
            row["contract_quality"]["mutants_killed"], "0/0",
            "`{name}` needs a load-bearing contract",
        );
    }

    let mut hostile_source = fs::read_to_string(root().join(source)).unwrap();
    hostile_source.push_str(
        r#"
fn fixed_intrusive_false_lifo_claim(first: usize, second: usize) -> bool
  req first < FIXED_INTRUSIVE_CAPACITY
    && second < FIXED_INTRUSIVE_CAPACITY
    && first != second
  ens result
  fx pure
{
  let empty: FixedIntrusiveList64 = fixed_intrusive_empty();
  match fixed_intrusive_push_back(empty, first) {
    FixedIntrusiveLink64::IntrusiveLinked64 { list: one } =>
      match fixed_intrusive_push_back(one, second) {
        FixedIntrusiveLink64::IntrusiveLinked64 { list: two } =>
          match fixed_intrusive_pop_front(two) {
            FixedIntrusivePop64::IntrusivePopped64 { list: _, node } =>
              node == second,
            FixedIntrusivePop64::IntrusiveEmpty64 { list: _ } => false,
            FixedIntrusivePop64::IntrusivePopCorrupt64 {
              list: _, node: _, reason: _,
            } => false,
          },
        FixedIntrusiveLink64::IntrusiveAlreadyLinked64 { list: _, node: _ } => false,
        FixedIntrusiveLink64::IntrusiveOutOfRange64 { list: _, node: _ } => false,
        FixedIntrusiveLink64::IntrusiveFull64 { list: _, node: _ } => false,
        FixedIntrusiveLink64::IntrusiveLinkCorrupt64 {
          list: _, node: _, reason: _,
        } => false,
      },
    FixedIntrusiveLink64::IntrusiveAlreadyLinked64 { list: _, node: _ } => false,
    FixedIntrusiveLink64::IntrusiveOutOfRange64 { list: _, node: _ } => false,
    FixedIntrusiveLink64::IntrusiveFull64 { list: _, node: _ } => false,
    FixedIntrusiveLink64::IntrusiveLinkCorrupt64 {
      list: _, node: _, reason: _,
    } => false,
  }
}

fn fixed_intrusive_false_duplicate_accept_claim(node: usize) -> bool
  req node < FIXED_INTRUSIVE_CAPACITY
  ens result
  fx pure
{
  let empty: FixedIntrusiveList64 = fixed_intrusive_empty();
  match fixed_intrusive_push_back(empty, node) {
    FixedIntrusiveLink64::IntrusiveLinked64 { list: one } =>
      match fixed_intrusive_push_back(one, node) {
        FixedIntrusiveLink64::IntrusiveLinked64 { list: _ } => true,
        FixedIntrusiveLink64::IntrusiveAlreadyLinked64 { list: _, node: _ } => false,
        FixedIntrusiveLink64::IntrusiveOutOfRange64 { list: _, node: _ } => false,
        FixedIntrusiveLink64::IntrusiveFull64 { list: _, node: _ } => false,
        FixedIntrusiveLink64::IntrusiveLinkCorrupt64 {
          list: _, node: _, reason: _,
        } => false,
      },
    FixedIntrusiveLink64::IntrusiveAlreadyLinked64 { list: _, node: _ } => false,
    FixedIntrusiveLink64::IntrusiveOutOfRange64 { list: _, node: _ } => false,
    FixedIntrusiveLink64::IntrusiveFull64 { list: _, node: _ } => false,
    FixedIntrusiveLink64::IntrusiveLinkCorrupt64 {
      list: _, node: _, reason: _,
    } => false,
  }
}

fn fixed_intrusive_clone_state_rejected(list: FixedIntrusiveList64) -> bool
  req true
  ens result
  fx pure
{
  let duplicate: FixedIntrusiveList64 = list.clone();
  duplicate.intrusive_len == list.intrusive_len
}

fn fixed_intrusive_false_unlink_preserves_present_claim(
  list: FixedIntrusiveList64,
  node: usize,
) -> bool
  req fixed_intrusive_unlink_reason_spec(&list, node) == 0
  ens result
  fx pure
{
  match fixed_intrusive_unlink(list, node) {
    FixedIntrusiveUnlink64::IntrusiveUnlinked64 {
      list: next,
      node: _,
    } => fixed_intrusive_contains(&next, node),
    FixedIntrusiveUnlink64::IntrusiveNotLinked64 { list: _, node: _ } => false,
    FixedIntrusiveUnlink64::IntrusiveUnlinkOutOfRange64 {
      list: _, node: _,
    } => false,
    FixedIntrusiveUnlink64::IntrusiveUnlinkCorrupt64 {
      list: _, node: _, reason: _,
    } => false,
  }
}
"#,
    );
    let hostile_path = temp.0.join("hostile-intrusive.th");
    fs::write(&hostile_path, hostile_source).unwrap();
    let hostile_path_s = hostile_path.to_string_lossy().to_string();
    let rejected = forge(&["check", &hostile_path_s, "--level", "l3", "--json"]);
    assert!(
        !rejected.status.success(),
        "LIFO/duplicate/clone/unlink claims unexpectedly certified",
    );
    let rejected_rows: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    for name in [
        "fixed_intrusive_false_lifo_claim",
        "fixed_intrusive_false_duplicate_accept_claim",
        "fixed_intrusive_clone_state_rejected",
        "fixed_intrusive_false_unlink_preserves_present_claim",
    ] {
        let row = rejected_rows
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing hostile intrusive row for `{name}`"));
        assert_ne!(
            row["level"], "L3",
            "hostile intrusive claim certified: {row}",
        );
    }

    let bundle = temp.0.join("intrusive.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "stdlib/kernel-primitives/intrusive.thpkg.json",
        "--level",
        "l3",
        "--export",
        "fixed_intrusive_unlink_middle_probe",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    for relative in [
        "evidence/thermite-package/manifest.json",
        "evidence/thermite-package/source-map.json",
        "evidence/thermite-package/source/collections/intrusive.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }
    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["package"]["name"], "thermite_fixed_intrusive");
    assert_eq!(
        plan["exports"][0]["thermite_name"],
        "fixed_intrusive_unlink_middle_probe",
    );

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    let tv_rows = tv["rows"].as_array().unwrap();
    assert_eq!(tv_rows.len(), 72);
    assert!(
        tv_rows.iter().all(|row| row["verdict"] == "faithful"),
        "intrusive bundle contains non-faithful TV rows: {tv}",
    );
    for (phase, label) in [
        ("body", "fixed_intrusive_unlink_middle_probe"),
        ("contract", "fixed_intrusive_push_back.ens#1"),
        ("contract", "fixed_intrusive_pop_front.ens#1"),
        ("contract", "fixed_intrusive_unlink.ens#1"),
        ("exec", "fixed_intrusive_push_back.let#1"),
        ("exec", "fixed_intrusive_pop_front.let#1"),
        ("exec", "fixed_intrusive_unlink.let#1"),
    ] {
        assert!(
            tv_rows
                .iter()
                .any(|row| row["phase"] == phase && row["label"] == label),
            "missing faithful {phase} TV row `{label}`: {tv}",
        );
    }

    let consumer_source = temp.0.join("consumer.rs");
    fs::write(
        &consumer_source,
        r#"fn main() {
    use thermite_fixed_intrusive::thermite_export_fixed_intrusive_unlink_middle_probe_v1;
    match thermite_export_fixed_intrusive_unlink_middle_probe_v1(5, 9, 13) {
        Ok(value) => assert!(value),
        Err(_) => panic!("valid distinct unlink slots rejected"),
    }
}
"#,
    )
    .unwrap();
    let artifact = bundle.join("artifact/libthermite_fixed_intrusive.rlib");
    let deps = bundle.join("artifact/deps");
    let consumer = temp.0.join("consumer");
    let compiled = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!("thermite_fixed_intrusive={}", artifact.display(),))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(&consumer)
        .output()
        .unwrap();
    assert_success(&compiled);
    assert_success(&Command::new(&consumer).output().unwrap());

    let bound_source = bundle.join("evidence/thermite-package/source/collections/intrusive.th");
    let original = fs::read_to_string(&bound_source).unwrap();
    let weakened = original.replacen(
        "#[opaque] struct FixedIntrusiveList64",
        "struct FixedIntrusiveList64",
        1,
    );
    assert_ne!(weakened, original, "intrusive opaque-tamper fixture missed",);
    fs::write(&bound_source, weakened).unwrap();
    let tampered = forge(&["verify-build", &bundle_s, "--replay", "--json"]);
    assert!(
        !tampered.status.success(),
        "removing the bound opaque intrusive-list barrier unexpectedly replayed",
    );
}
