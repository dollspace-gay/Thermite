//! Primitive-only acceptance for the allocation-free fixed collections.
//!
//! Both algorithms are authored in Thermite. The package contains no platform
//! boundary and no Rust implementation: Forge proves every source item at L3,
//! rejects deliberately false collection claims, builds a freestanding strict
//! export, and replays the receipt-bound two-module source closure.

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
            "thermite_fixed_collections_{name}_{}_{}",
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

fn checked_rows(source: &str) -> Vec<serde_json::Value> {
    let checked = forge(&["check", source, "--level", "l3", "--json"]);
    assert_success(&checked);
    serde_json::from_slice::<serde_json::Value>(&checked.stdout)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

fn assert_l3_functions(rows: &[serde_json::Value], names: &[&str]) {
    assert!(
        rows.iter().all(|row| row["level"] == "L3"),
        "a collection source item failed L3: {rows:?}"
    );
    assert!(
        rows.iter().all(|row| row["boundary"] == false),
        "fixed collections must not contain platform boundaries: {rows:?}"
    );
    for name in names {
        let row = rows
            .iter()
            .find(|row| row["item"] == *name)
            .unwrap_or_else(|| panic!("missing collection certificate for `{name}`"));
        assert_ne!(
            row["contract_quality"]["mutants_killed"], "0/0",
            "`{name}` must have a load-bearing contract"
        );
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

fn assert_false_claim_rejected(source: &str, name: &str, temp: &TempDir) {
    let path = temp.0.join(format!("{name}.th"));
    fs::write(&path, source).unwrap();
    let path_s = path.to_string_lossy().to_string();
    let rejected = forge(&["check", &path_s, "--level", "l3", "--json"]);
    assert!(
        !rejected.status.success(),
        "`{name}` unexpectedly certified"
    );
    let rows: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["item"] == name)
        .unwrap_or_else(|| panic!("missing rejection row for `{name}`"));
    assert_ne!(row["level"], "L3", "false claim certified: {row}");
}

#[test]
fn bitmap_and_ring_are_l3_freestanding_receipt_bound_primitives() {
    let temp = TempDir::new("package");
    let bitmap_source = "stdlib/kernel-primitives/collections/bitmap.th";
    let ring_source = "stdlib/kernel-primitives/collections/ring.th";

    let bitmap_rows = checked_rows(bitmap_source);
    assert_l3_functions(
        &bitmap_rows,
        &[
            "fixed_bitmap_empty",
            "fixed_bitmap_contains",
            "fixed_bitmap_insert",
            "fixed_bitmap_remove",
            "fixed_bitmap_set_to",
            "fixed_bitmap_insert_remove_probe",
            "fixed_bitmap_set_to_probe",
        ],
    );
    assert_eq!(mutation_total(&bitmap_rows), (14, 16));

    let ring_rows = checked_rows(ring_source);
    assert_l3_functions(
        &ring_rows,
        &[
            "fixed_ring_advance",
            "fixed_ring_tail",
            "fixed_ring_empty",
            "fixed_ring_peek",
            "fixed_ring_push",
            "fixed_ring_pop",
            "fixed_ring_fifo_probe",
            "fixed_ring_wrap_probe",
            "fixed_ring_full_reject_probe",
        ],
    );
    assert_eq!(mutation_total(&ring_rows), (64, 71));

    let mut false_bitmap = fs::read_to_string(root().join(bitmap_source)).unwrap();
    false_bitmap.push_str(
        r#"
fn fixed_bitmap_false_absence_claim(bit: usize) -> bool
  req bit < FIXED_BITMAP_BITS
  ens !result
  fx pure
{
  let empty: FixedBitmap256 = fixed_bitmap_empty();
  let inserted: FixedBitmap256 = fixed_bitmap_insert(empty, bit);
  fixed_bitmap_contains(&inserted, bit)
}
"#,
    );
    assert_false_claim_rejected(&false_bitmap, "fixed_bitmap_false_absence_claim", &temp);

    let mut false_ring = fs::read_to_string(root().join(ring_source)).unwrap();
    false_ring.push_str(
        r#"
fn fixed_ring_false_lifo_claim(first: u64, second: u64) -> u64
  req first != second
  ens result == second
  fx pure
{
  let empty: FixedRing64 = fixed_ring_empty();
  match fixed_ring_push(empty, first) {
    FixedRingPush64::Pushed64 { ring: one } =>
      match fixed_ring_push(one, second) {
        FixedRingPush64::Pushed64 { ring: two } =>
          match fixed_ring_pop(two) {
            FixedRingPop64::Popped64 { ring: _, value } => value,
            FixedRingPop64::RingEmpty64 { ring: _ } => second,
          },
        FixedRingPush64::RingFull64 { ring: _, value: _ } => second,
      },
    FixedRingPush64::RingFull64 { ring: _, value: _ } => second,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_ring, "fixed_ring_false_lifo_claim", &temp);

    let bundle = temp.0.join("collections.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "stdlib/kernel-primitives/collections.thpkg.json",
        "--level",
        "l3",
        "--export",
        "fixed_ring_advance",
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
        "evidence/thermite-package/source/collections/bitmap.th",
        "evidence/thermite-package/source/collections/ring.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["package"]["name"], "thermite_fixed_collections");
    assert_eq!(plan["exports"][0]["thermite_name"], "fixed_ring_advance");

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
        "strict collection surface contains non-faithful TV rows: {tv}"
    );

    let bound_ring = bundle.join("evidence/thermite-package/source/collections/ring.th");
    let mut tampered = fs::read_to_string(&bound_ring).unwrap();
    tampered.push_str("\n// receipt tamper\n");
    fs::write(&bound_ring, tampered).unwrap();
    let tamper_rejected = forge(&["verify-build", &bundle_s, "--json"]);
    assert!(
        !tamper_rejected.status.success(),
        "tampered collection source unexpectedly validated"
    );
}
