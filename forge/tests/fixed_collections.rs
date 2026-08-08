//! Primitive-only acceptance for the allocation-free fixed collections.
//!
//! All five collection families are authored in Thermite. The package contains no platform
//! boundary and no Rust implementation: Forge proves every source item at L3,
//! rejects deliberately false collection claims, builds a freestanding strict
//! export, and replays the receipt-bound five-module source closure.

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
    assert_false_claims_rejected(source, &[name], temp);
}

fn assert_false_claims_rejected(source: &str, names: &[&str], temp: &TempDir) {
    let label = names.join("-");
    let path = temp.0.join(format!("{label}.th"));
    fs::write(&path, source).unwrap();
    let path_s = path.to_string_lossy().to_string();
    let rejected = forge(&["check", &path_s, "--level", "l3", "--json"]);
    assert!(
        !rejected.status.success(),
        "false claims unexpectedly certified: {names:?}"
    );
    let rows: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    for name in names {
        let row = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["item"] == *name)
            .unwrap_or_else(|| panic!("missing rejection row for `{name}`"));
        assert_ne!(row["level"], "L3", "false claim certified: {row}");
    }
}

#[test]
fn fixed_collections_are_l3_freestanding_receipt_bound_primitives() {
    let temp = TempDir::new("package");
    let bitmap_source = "stdlib/kernel-primitives/collections/bitmap.th";
    let direct_map_source = "stdlib/kernel-primitives/collections/direct_map.th";
    let open_map_source = "stdlib/kernel-primitives/collections/open_map.th";
    let ring_source = "stdlib/kernel-primitives/collections/ring.th";
    let vector_source = "stdlib/kernel-primitives/collections/vector.th";

    let bitmap_rows = checked_rows(bitmap_source);
    assert_l3_functions(
        &bitmap_rows,
        &[
            "fixed_bitmap_word",
            "fixed_bitmap_offset",
            "fixed_bitmap_empty",
            "fixed_bitmap_contains",
            "fixed_bitmap_insert",
            "fixed_bitmap_remove",
            "fixed_bitmap_set_to",
            "fixed_bitmap_count",
            "fixed_bitmap_first_set_from",
            "fixed_bitmap_first_set",
            "fixed_bitmap_union",
            "fixed_bitmap_intersection",
            "fixed_bitmap_difference",
            "fixed_bitmap_insert_remove_probe",
            "fixed_bitmap_set_to_probe",
            "fixed_bitmap_word_boundary_probe",
        ],
    );
    assert_eq!(bitmap_rows.len(), 36);
    // Seven functions return `FixedBitmap256`: `fixed_bitmap_empty` and the six
    // owned transitions. Each gained one early-return zero mutant when
    // `zero_value_for` in `mutation.rs` learned to zero a fixed array, so a
    // record whose fields are all zero-able has a zero as well. All seven die on
    // `ens result.capacity == FIXED_BITMAP_BITS`, because the zeroed record
    // carries `capacity == 0`. The pin therefore rises by seven in both places.
    assert_eq!(mutation_total(&bitmap_rows), (114, 121));

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
    // The remaining four collections gained early-return zero mutants from the
    // same `zero_value_for` change, one per function that returns the collection
    // record. Every such mutant on an EMPTY CONSTRUCTOR survives, because
    // `FixedRing64`, `FixedVec64`, `FixedDirectMap64`, and `FixedOpenMap64` carry
    // no field with a nonzero invariant: each constructor's body is literally its
    // own zero value, so the mutant computes the same result and no contract can
    // separate them. `fixed_open_map_empty` is the clearest case, since
    // `OPEN_MAP_EMPTY` is 0. The bitmap above is the control: its `capacity` must
    // equal `FIXED_BITMAP_BITS`, so its zero is distinguishable and all seven of
    // its mutants die.
    //
    // A mutant on a TRANSITION dies, because the transition's postconditions
    // relate the result to its input. Vector is the only collection here with
    // both shapes: two functions return `FixedVec64`, so it gained two mutants,
    // of which the constructor's survives and the transition's dies. Ring,
    // direct map, and open map have one returning function each.
    assert_eq!(mutation_total(&ring_rows), (64, 72));

    let vector_rows = checked_rows(vector_source);
    assert_l3_functions(
        &vector_rows,
        &[
            "fixed_vec_empty",
            "fixed_vec_get",
            "fixed_vec_set",
            "fixed_vec_push",
            "fixed_vec_pop",
            "fixed_vec_random_access_probe",
            "fixed_vec_lifo_probe",
            "fixed_vec_set_probe",
        ],
    );
    assert_eq!(mutation_total(&vector_rows), (46, 51));

    let direct_map_rows = checked_rows(direct_map_source);
    assert_l3_functions(
        &direct_map_rows,
        &[
            "fixed_direct_map_slot",
            "fixed_direct_map_empty_for",
            "fixed_direct_map_lookup",
            "fixed_direct_map_insert",
            "fixed_direct_map_remove",
            "fixed_direct_map_insert_lookup_probe",
            "fixed_direct_map_replace_probe",
            "fixed_direct_map_collision_probe",
            "fixed_direct_map_remove_probe",
        ],
    );
    assert_eq!(mutation_total(&direct_map_rows), (54, 59));

    let open_map_rows = checked_rows(open_map_source);
    assert_l3_functions(
        &open_map_rows,
        &[
            "fixed_open_map_home",
            "fixed_open_map_next",
            "fixed_open_map_empty",
            "fixed_open_map_find",
            "fixed_open_map_search",
            "fixed_open_map_lookup",
            "fixed_open_map_insert",
            "fixed_open_map_remove",
            "fixed_open_map_insert_lookup_probe",
            "fixed_open_map_collision_probe",
            "fixed_open_map_delete_reuse_probe",
        ],
    );
    assert_eq!(open_map_rows.len(), 43);
    assert_eq!(mutation_total(&open_map_rows), (72, 81));

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

fn fixed_bitmap_false_count_bound_claim(bitmap: &FixedBitmap256) -> u64
  req fixed_bitmap_wf_spec(bitmap)
  ens result > FIXED_BITMAP_BITS as u64
  fx pure
{
  fixed_bitmap_count(bitmap)
}

fn fixed_bitmap_false_union_absence_claim(
  left: FixedBitmap256,
  right: &FixedBitmap256,
  bit: usize,
) -> FixedBitmap256
  req fixed_bitmap_wf_spec(&left)
    && fixed_bitmap_wf_spec(right)
    && bit < FIXED_BITMAP_BITS
    && fixed_bitmap_contains_spec(&left, bit)
  ens !fixed_bitmap_contains_spec(&result, bit)
  fx pure
{
  fixed_bitmap_union(left, right)
}
"#,
    );
    assert_false_claims_rejected(
        &false_bitmap,
        &[
            "fixed_bitmap_false_absence_claim",
            "fixed_bitmap_false_count_bound_claim",
            "fixed_bitmap_false_union_absence_claim",
        ],
        &temp,
    );

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

    let mut false_vector = fs::read_to_string(root().join(vector_source)).unwrap();
    false_vector.push_str(
        r#"
fn fixed_vec_false_fifo_claim(first: u64, second: u64) -> u64
  req first != second
  ens result == first
  fx pure
{
  let empty: FixedVec64 = fixed_vec_empty();
  match fixed_vec_push(empty, first) {
    FixedVecPush64::VecPushed64 { vector: one } =>
      match fixed_vec_push(one, second) {
        FixedVecPush64::VecPushed64 { vector: two } =>
          match fixed_vec_pop(two) {
            FixedVecPop64::VecPopped64 { vector: _, value } => value,
            FixedVecPop64::VecEmpty64 { vector: _ } => first,
          },
        FixedVecPush64::VecFull64 { vector: _, value: _ } => first,
      },
    FixedVecPush64::VecFull64 { vector: _, value: _ } => first,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_vector, "fixed_vec_false_fifo_claim", &temp);

    let mut false_map = fs::read_to_string(root().join(direct_map_source)).unwrap();
    false_map.push_str(
        r#"
fn fixed_direct_map_false_missing_claim(key: usize, value: u64) -> bool
  req true
  ens result
  fx pure
{
  let empty: FixedDirectMap64 = fixed_direct_map_empty_for(key);
  match fixed_direct_map_insert(empty, key, value) {
    FixedMapInsert64::MapAdded64 { map } =>
      match fixed_direct_map_lookup(&map, key) {
        FixedMapLookup64::MapFound64 { value: _ } => false,
        FixedMapLookup64::MapVacant64 => true,
        FixedMapLookup64::MapLookupCollision64 { stored_key: _ } => true,
      },
    FixedMapInsert64::MapReplaced64 { map: _, old_value: _ } => true,
    FixedMapInsert64::MapInsertCollision64 {
      map: _, key: _, value: _, stored_key: _,
    } => true,
    FixedMapInsert64::MapInsertCountInvalid64 { map: _, key: _, value: _ } => true,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_map, "fixed_direct_map_false_missing_claim", &temp);

    let mut false_open_map = fs::read_to_string(root().join(open_map_source)).unwrap();
    false_open_map.push_str(
        r#"
fn fixed_open_map_false_collision_slot_claim(first: u64) -> bool
  req true
  ens result
  fx pure
{
  let empty: FixedOpenMap64 = fixed_open_map_empty();
  match fixed_open_map_insert(empty, 0, first) {
    FixedOpenMapInsert64::OpenMapAdded64 { map: one, slot: _ } =>
      match fixed_open_map_search(&one, 64, 0, 2, FIXED_OPEN_MAP_CAPACITY) {
        FixedOpenMapSearch64::OpenMapExisting64 { slot: _ } => true,
        FixedOpenMapSearch64::OpenMapVacant64 { slot } => slot == 0,
        FixedOpenMapSearch64::OpenMapFull64 => true,
      },
    FixedOpenMapInsert64::OpenMapReplaced64 {
      map: _, slot: _, old_value: _,
    } => true,
    FixedOpenMapInsert64::OpenMapInsertFull64 {
      map: _, key: _, value: _,
    } => true,
  }
}
"#,
    );
    assert_false_claim_rejected(
        &false_open_map,
        "fixed_open_map_false_collision_slot_claim",
        &temp,
    );

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
        "evidence/thermite-package/source/collections/direct_map.th",
        "evidence/thermite-package/source/collections/open_map.th",
        "evidence/thermite-package/source/collections/ring.th",
        "evidence/thermite-package/source/collections/vector.th",
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

    let bound_map = bundle.join("evidence/thermite-package/source/collections/direct_map.th");
    let mut tampered = fs::read_to_string(&bound_map).unwrap();
    tampered.push_str("\n// receipt tamper\n");
    fs::write(&bound_map, tampered).unwrap();
    let tamper_rejected = forge(&["verify-build", &bundle_s, "--json"]);
    assert!(
        !tamper_rejected.status.success(),
        "tampered collection source unexpectedly validated"
    );
}
