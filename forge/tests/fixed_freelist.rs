//! Primitive-only acceptance for the allocation-free, duplicate-safe freelist.
//!
//! The implementation and fail-closed push/pop mechanics are authored in
//! Thermite. Every bodyful item must certify at L3; the package has no platform
//! boundary and no parallel Rust implementation.

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
        String::from_utf8_lossy(&output.stderr)
    );
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_fixed_freelist_{name}_{}_{}",
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
fn fixed_freelist_is_l3_fail_closed_and_receipt_bound() {
    let source = "stdlib/kernel-primitives/collections/freelist.th";
    let temp = TempDir::new("acceptance");

    let checked = forge(&["check", source, "--level", "l3", "--json"]);
    assert_success(&checked);
    let rows: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 23);
    assert!(
        rows.iter()
            .all(|row| row["level"] == "L3" && row["boundary"] == false),
        "a freelist item fell below boundary-free L3: {rows:?}"
    );
    // `fixed_freelist_empty` returns `FixedFreelist64`, whose array fields became
    // zero-able when `zero_value_for` in `mutation.rs` learned to zero a fixed
    // array. It gained one early-return zero mutant, which dies on the declared
    // length. The two result enums have no zero, so they gained none.
    assert_eq!(mutation_total(rows), (50, 54));
    for name in [
        "fixed_freelist_empty",
        "fixed_freelist_contains",
        "fixed_freelist_push_at",
        "fixed_freelist_push",
        "fixed_freelist_pop_live",
        "fixed_freelist_pop",
        "fixed_freelist_lifo_probe",
        "fixed_freelist_duplicate_probe",
        "fixed_freelist_reuse_probe",
    ] {
        let row = rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing freelist certificate for `{name}`"));
        assert_ne!(
            row["contract_quality"]["mutants_killed"], "0/0",
            "`{name}` needs a load-bearing contract"
        );
    }

    let mut hostile_source = fs::read_to_string(root().join(source)).unwrap();
    hostile_source.push_str(
        r#"
fn fixed_freelist_false_duplicate_accept_claim(
  list: FixedFreelist64,
  node: usize,
) -> bool
  req node < FIXED_FREELIST_CAPACITY
    && fixed_freelist_contains_spec(&list, node)
  ens result
  fx pure
{
  match fixed_freelist_push(list, node) {
    FixedFreelistPush64::FreelistPushed64 { list: _ } => true,
    FixedFreelistPush64::FreelistDuplicate64 { list: _, node: _ } => false,
    FixedFreelistPush64::FreelistOutOfRange64 { list: _, node: _ } => false,
    FixedFreelistPush64::FreelistFull64 { list: _, node: _ } => false,
  }
}

fn fixed_freelist_false_out_of_range_present_claim(
  list: &FixedFreelist64,
  node: usize,
) -> bool
  req node >= FIXED_FREELIST_CAPACITY
  ens result
  fx pure
{
  fixed_freelist_contains(list, node)
}

fn fixed_freelist_clone_state_rejected(list: FixedFreelist64) -> bool
  req true
  ens result
  fx pure
{
  let duplicate: FixedFreelist64 = list.clone();
  fixed_freelist_is_empty(&duplicate) == fixed_freelist_is_empty(&list)
}
"#,
    );
    let hostile_path = temp.0.join("hostile-freelist.th");
    fs::write(&hostile_path, hostile_source).unwrap();
    let hostile_path_s = hostile_path.to_string_lossy().to_string();
    let rejected = forge(&["check", &hostile_path_s, "--level", "l3", "--json"]);
    assert!(
        !rejected.status.success(),
        "duplicate/out-of-range/clone claims unexpectedly certified"
    );
    let rejected_rows: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    for name in [
        "fixed_freelist_false_duplicate_accept_claim",
        "fixed_freelist_false_out_of_range_present_claim",
        "fixed_freelist_clone_state_rejected",
    ] {
        let row = rejected_rows
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing hostile freelist row for `{name}`"));
        assert_ne!(
            row["level"], "L3",
            "hostile freelist claim certified: {row}"
        );
    }

    let bundle = temp.0.join("freelist.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "stdlib/kernel-primitives/freelist.thpkg.json",
        "--level",
        "l3",
        "--export",
        "fixed_freelist_lifo_probe",
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
        "evidence/thermite-package/source/collections/freelist.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }
    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["package"]["name"], "thermite_fixed_freelist");
    assert_eq!(
        plan["exports"][0]["thermite_name"],
        "fixed_freelist_lifo_probe"
    );

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    assert!(
        tv["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["verdict"] == "faithful"),
        "freelist bundle contains non-faithful TV rows: {tv}"
    );

    let bound_source = bundle.join("evidence/thermite-package/source/collections/freelist.th");
    let original = fs::read_to_string(&bound_source).unwrap();
    let weakened = original.replacen(
        "#[opaque] struct FixedFreelist64",
        "struct FixedFreelist64",
        1,
    );
    assert_ne!(weakened, original, "freelist opaque-tamper fixture missed");
    fs::write(&bound_source, weakened).unwrap();
    let tampered = forge(&["verify-build", &bundle_s, "--replay", "--json"]);
    assert!(
        !tampered.status.success(),
        "removing the bound opaque freelist barrier unexpectedly replayed"
    );
}
