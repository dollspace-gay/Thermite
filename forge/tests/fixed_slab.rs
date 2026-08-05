//! Primitive-only acceptance for the allocation-free generation-safe slab.
//!
//! The implementation and policy-free allocation/release mechanics are authored
//! in Thermite. Every bodyful item must certify at L3; the package has no platform
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
            "thermite_fixed_slab_{name}_{}_{}",
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
fn fixed_slab_is_l3_generation_safe_and_receipt_bound() {
    let source = "stdlib/kernel-primitives/collections/slab.th";
    let temp = TempDir::new("acceptance");

    let checked = forge(&["check", source, "--level", "l3", "--json"]);
    assert_success(&checked);
    let rows: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 36);
    assert!(
        rows.iter()
            .all(|row| row["level"] == "L3" && row["boundary"] == false),
        "a slab item fell below boundary-free L3: {rows:?}"
    );
    assert_eq!(mutation_total(rows), (70, 73));
    for name in [
        "fixed_slab_empty",
        "fixed_slab_handle_live",
        "fixed_slab_find_free",
        "fixed_slab_get",
        "fixed_slab_allocate_found",
        "fixed_slab_allocate",
        "fixed_slab_release",
        "fixed_slab_allocate_get_probe",
        "fixed_slab_allocate_release_probe",
    ] {
        let row = rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing slab certificate for `{name}`"));
        assert_ne!(
            row["contract_quality"]["mutants_killed"], "0/0",
            "`{name}` needs a load-bearing contract"
        );
    }

    let mut hostile_source = fs::read_to_string(root().join(source)).unwrap();
    hostile_source.push_str(
        r#"
fn fixed_slab_false_released_state_live_claim(
  slab: &FixedSlab64,
  slot: usize,
  generation: u64,
) -> bool
  req slot < FIXED_SLAB_CAPACITY
    && !slab.slab_used[slot]
    && slab.slab_generation[slot] == generation
    && generation != 0
  ens result
  fx pure
{
  let stale: FixedSlabHandle64 = FixedSlabHandle64 {
    slab_slot: slot,
    slab_generation: generation,
  };
  fixed_slab_handle_live(slab, &stale)
}

fn fixed_slab_false_released_handle_live_claim(
  slab: FixedSlab64,
  handle: FixedSlabHandle64,
) -> bool
  req fixed_slab_handle_live_spec(&slab, &handle)
  ens result
  fx pure
{
  let slot: usize = handle.slab_slot;
  let generation: u64 = handle.slab_generation;
  match fixed_slab_release(slab, handle) {
    FixedSlabRelease64::SlabReleased64 { slab: released, value: _ } =>
      fixed_slab_false_released_state_live_claim(
        &released,
        slot,
        generation,
      ),
    FixedSlabRelease64::SlabReleaseRejected64 {
      slab: _, handle: _, reason: _,
    } => true,
  }
}

fn fixed_slab_clone_handle_rejected(handle: FixedSlabHandle64) -> bool
  req true
  ens result
  fx pure
{
  let duplicate: FixedSlabHandle64 = handle.clone();
  duplicate.slab_slot == handle.slab_slot
}
"#,
    );
    let hostile_path = temp.0.join("hostile-slab.th");
    fs::write(&hostile_path, hostile_source).unwrap();
    let hostile_path_s = hostile_path.to_string_lossy().to_string();
    let rejected = forge(&["check", &hostile_path_s, "--level", "l3", "--json"]);
    assert!(
        !rejected.status.success(),
        "stale-live or clone claim unexpectedly certified"
    );
    let rejected_rows: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    for name in [
        "fixed_slab_false_released_state_live_claim",
        "fixed_slab_false_released_handle_live_claim",
        "fixed_slab_clone_handle_rejected",
    ] {
        let row = rejected_rows
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing hostile slab row for `{name}`"));
        assert_ne!(row["level"], "L3", "hostile slab claim certified: {row}");
    }

    let bundle = temp.0.join("slab.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "stdlib/kernel-primitives/slab.thpkg.json",
        "--level",
        "l3",
        "--export",
        "fixed_slab_allocate_get_probe",
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
        "evidence/thermite-package/source/collections/slab.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }
    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["package"]["name"], "thermite_fixed_slab");
    assert_eq!(
        plan["exports"][0]["thermite_name"],
        "fixed_slab_allocate_get_probe"
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
        "slab bundle contains non-faithful TV rows: {tv}"
    );

    let bound_source = bundle.join("evidence/thermite-package/source/collections/slab.th");
    let original = fs::read_to_string(&bound_source).unwrap();
    let weakened = original.replacen("#[opaque] struct FixedSlab64", "struct FixedSlab64", 1);
    assert_ne!(weakened, original, "slab opaque-tamper fixture missed");
    fs::write(&bound_source, weakened).unwrap();
    let tampered = forge(&["verify-build", &bundle_s, "--replay", "--json"]);
    assert!(
        !tampered.status.success(),
        "removing the bound opaque slab barrier unexpectedly replayed"
    );
}
