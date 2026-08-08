//! Primitive-only acceptance for the generic platform-operation package.
//!
//! The package contains no kernel policy and no architecture implementation.
//! Every executable/model/type row must prove at L3; the exact sub-L3 set is
//! the bodyless machine boundary inventory that a consumer must directly refine.

use std::collections::{BTreeMap, BTreeSet};
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
            "thermite_platform_primitives_{name}_{}_{}",
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

fn expected_family_counts() -> BTreeMap<&'static str, usize> {
    BTreeMap::from([
        ("thermite::boot::", 2),
        ("thermite::clock::", 3),
        ("thermite::cpu::", 8),
        ("thermite::dma::", 4),
        ("thermite::entropy::", 1),
        ("thermite::irq::", 7),
        ("thermite::memory::", 18),
        ("thermite::mmio::", 10),
        ("thermite::pio::", 7),
        ("thermite::power::", 2),
        ("thermite::runtime::", 6),
        ("thermite::smp::", 4),
        ("thermite::trap::", 2),
    ])
}

fn expected_effect(target: &str) -> &'static str {
    if target.starts_with("thermite::boot::")
        || target.starts_with("thermite::runtime::panic")
        || target.starts_with("thermite::runtime::contract")
        || target.starts_with("thermite::runtime::allocation")
    {
        "platform(boot)"
    } else if target.starts_with("thermite::runtime::") || target.starts_with("thermite::memory::")
    {
        "platform(memory)"
    } else if target.starts_with("thermite::mmio::") {
        "platform(mmio)"
    } else if target.starts_with("thermite::pio::") {
        "platform(pio)"
    } else if target.starts_with("thermite::cpu::") {
        "platform(cpu)"
    } else if target.starts_with("thermite::irq::") || target.starts_with("thermite::trap::") {
        "platform(irq)"
    } else if target.starts_with("thermite::smp::") {
        "platform(smp)"
    } else if target.starts_with("thermite::dma::") {
        "platform(dma)"
    } else if target.starts_with("thermite::clock::") {
        "platform(clock)"
    } else if target.starts_with("thermite::entropy::") {
        "platform(entropy)"
    } else if target.starts_with("thermite::power::") {
        "platform(power)"
    } else {
        panic!("unclassified platform boundary `{target}`")
    }
}

#[test]
fn platform_declarations_are_the_only_sub_l3_rows_and_models_are_receipt_bound() {
    let source = "stdlib/kernel-primitives/platform/api.th";
    let source_text = fs::read_to_string(root().join(source)).unwrap();
    let parsed = thermite_syntax::parse(&source_text);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let declared_boundaries: Vec<_> = parsed
        .program
        .items
        .iter()
        .filter_map(|item| match item {
            thermite_syntax::Item::Fn(function) if function.boundary.is_some() => Some(function),
            _ => None,
        })
        .collect();
    assert_eq!(declared_boundaries.len(), 74);
    assert!(declared_boundaries
        .iter()
        .all(|function| function.body.is_none()));

    let checked = forge(&[
        "check", source, "--level", "l3", "--engine", "verus", "--json",
    ]);
    assert_success(&checked);
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(rows.len(), 129);

    let boundary_rows: Vec<&serde_json::Value> =
        rows.iter().filter(|row| row["boundary"] == true).collect();
    assert_eq!(boundary_rows.len(), 74);
    assert_eq!(rows.len() - boundary_rows.len(), 55);

    let mut targets = BTreeSet::new();
    let mut observed_family_counts = BTreeMap::new();
    for row in &boundary_rows {
        assert_eq!(
            row["level"], "L1",
            "machine declaration changed level: {row}"
        );
        let target = row["boundary_target"].as_str().unwrap();
        assert!(
            targets.insert(target),
            "duplicate boundary target `{target}`"
        );
        let family = expected_family_counts()
            .keys()
            .copied()
            .find(|prefix| target.starts_with(prefix))
            .unwrap_or_else(|| panic!("unexpected platform family for `{target}`"));
        *observed_family_counts.entry(family).or_insert(0usize) += 1;
        let effects: Vec<&str> = row["effects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|effect| effect.as_str().unwrap())
            .collect();
        assert!(
            effects.contains(&expected_effect(target)),
            "boundary `{target}` lacks its exact platform effect: {effects:?}",
        );
    }
    assert_eq!(observed_family_counts, expected_family_counts());

    for row in rows.iter().filter(|row| row["boundary"] == false) {
        assert_eq!(
            row["level"], "L3",
            "a bodyful/model/type platform primitive fell below L3: {row}",
        );
    }
    for name in [
        "platform_width_legal",
        "raw_range_legal",
        "raw_aligned",
        "mmio_range_legal",
        "mmio_aligned",
    ] {
        let row = rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing platform helper `{name}`"));
        assert_eq!(row["level"], "L3");
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }

    for terminal in [
        "boot_entry_transfer",
        "runtime_panic_terminal",
        "runtime_contract_failure_terminal",
        "runtime_allocation_failure_terminal",
        "trap_context_return",
        "power_reboot_terminal",
        "power_off_terminal",
    ] {
        let row = rows.iter().find(|row| row["item"] == terminal).unwrap();
        assert!(row["effects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|effect| effect == "diverge"));
    }

    let temp = TempDir::new("bundle");
    let bundle = temp.0.join("platform.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "stdlib/kernel-primitives/platform.thpkg.json",
        "--level",
        "l3",
        "--export",
        "platform_width_legal",
        "--crate-name",
        "thermite_platform_primitives",
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
        "evidence/thermite-package/source/platform/api.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }
    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["package"]["name"], "thermite_platform_primitives");
    assert_eq!(plan["exports"][0]["thermite_name"], "platform_width_legal");

    let mut false_claim = source_text;
    false_claim.push_str(
        r#"
fn platform_width_three_is_legal() -> bool
  req true
  ens result
  fx pure
{
  platform_width_legal(3)
}
"#,
    );
    let false_path = temp.0.join("false-width.th");
    fs::write(&false_path, false_claim).unwrap();
    let false_path_s = false_path.to_string_lossy().to_string();
    let rejected = forge(&[
        "check",
        &false_path_s,
        "--level",
        "l3",
        "--engine",
        "verus",
        "--json",
    ]);
    assert!(!rejected.status.success());
    let rejected_rows: Vec<serde_json::Value> = serde_json::from_slice(&rejected.stdout).unwrap();
    let false_row = rejected_rows
        .iter()
        .find(|row| row["item"] == "platform_width_three_is_legal")
        .unwrap();
    assert_ne!(false_row["level"], "L3");
}
