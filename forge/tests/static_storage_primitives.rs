//! Primitive-only acceptance for allocation-free static-storage ownership.
//!
//! The reusable claim/commit/release algorithms are authored in Thermite and
//! must all certify at L3.  Only the authority door and the operation that
//! physically fills consumer-owned memory remain bodyless L1 declarations;
//! their exact machine implementations are deliberately consumer-refined.

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
            "thermite_static_storage_primitives_{name}_{}_{}",
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

#[test]
fn static_storage_lifecycle_is_l3_sealed_replayable_and_executable() {
    let source = "stdlib/kernel-primitives/storage/static_storage.th";
    let temp = TempDir::new("lifecycle");
    let checked = forge(&["check", source, "--level", "l3", "--json"]);
    assert_success(&checked);
    let certificates: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let certificates = certificates.as_array().unwrap();

    let expected_boundaries = [
        (
            "static_storage_authority",
            "thermite::storage::static_authority",
        ),
        ("static_storage_fill_bytes", "thermite::storage::fill_bytes"),
    ];
    for (name, target) in expected_boundaries {
        let row = certificates
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing boundary certificate `{name}`"));
        assert_eq!(row["level"], "L1");
        assert_eq!(row["boundary"], true);
        assert_eq!(row["boundary_target"], target);
    }

    for row in certificates {
        let name = row["item"].as_str().unwrap();
        if expected_boundaries
            .iter()
            .any(|(boundary, _)| *boundary == name)
        {
            continue;
        }
        assert_eq!(
            row["level"], "L3",
            "an in-language static-storage primitive fell below L3: {row}",
        );
        assert_eq!(
            row["boundary"], false,
            "only the two irreducible platform doors may be boundaries: {row}",
        );
    }

    for name in [
        "static_storage_capacity_valid",
        "static_storage_claim_reason",
        "static_storage_commit_reason",
        "static_storage_ledger_init",
        "static_storage_lease_live",
        "static_storage_witness_matches",
        "static_storage_claim_at",
        "static_storage_commit",
        "static_storage_release_uninitialized",
        "static_storage_zero_capacity_rejected",
        "static_storage_release_invalidates_lease",
        "static_storage_stale_witness_rejected",
        "static_storage_commit_after_claim",
        "static_storage_reuse_after_claim",
        "static_storage_lifecycle_probe",
        "static_storage_slot_reuse_probe",
    ] {
        let row = certificates
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing certificate for `{name}`"));
        assert_eq!(row["level"], "L3", "`{name}` did not prove at L3: {row}");
        assert_ne!(
            row["contract_quality"]["mutants_killed"], "0/0",
            "`{name}` must carry a non-vacuous mutation score",
        );
    }

    let mut adversarial_source = fs::read_to_string(root().join(source)).unwrap();
    adversarial_source.push_str(
        r#"
fn static_storage_duplicate_ledger_rejected(
  ledger: StaticStorageLedger,
  slot: usize,
) -> bool
  req slot < STATIC_STORAGE_SLOT_COUNT
  ens result
  fx pure
{
  let first: StaticStorageClaim = static_storage_claim_at(ledger, slot, 1);
  let second: StaticStorageClaim = static_storage_claim_at(ledger, slot, 1);
  true
}

fn static_storage_clone_lease_rejected(lease: StaticStorageLease) -> bool
  req true
  ens result
  fx pure
{
  let duplicate: StaticStorageLease = lease.clone();
  duplicate.slot == lease.slot
}
"#,
    );
    let adversarial_path = temp.0.join("duplicate-authority.th");
    fs::write(&adversarial_path, adversarial_source).unwrap();
    let adversarial_path_s = adversarial_path.to_string_lossy().to_string();
    let rejected = forge(&["check", &adversarial_path_s, "--level", "l3", "--json"]);
    assert!(
        !rejected.status.success(),
        "duplicating the ledger or cloning a lease unexpectedly certified",
    );
    let rejected_rows: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    for name in [
        "static_storage_duplicate_ledger_rejected",
        "static_storage_clone_lease_rejected",
    ] {
        let row = rejected_rows
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing adversarial certificate for `{name}`"));
        assert_ne!(row["level"], "L3", "`{name}` duplicated authority: {row}");
    }

    let opaque_package = temp.0.join("opaque-package");
    fs::create_dir(&opaque_package).unwrap();
    fs::write(
        opaque_package.join("static_storage.th"),
        fs::read_to_string(root().join(source)).unwrap(),
    )
    .unwrap();
    fs::write(
        opaque_package.join("api.th"),
        r#"fn forge_static_storage_lease(
  authority: usize,
  slot: usize,
  generation: u64,
  capacity: usize,
) -> StaticStorageLease
  req true
  ens result.authority == authority
    && result.slot == slot
    && result.generation == generation
    && result.capacity == capacity
  fx pure
{
  StaticStorageLease {
    authority: authority,
    slot: slot,
    generation: generation,
    capacity: capacity,
  }
}
"#,
    )
    .unwrap();
    fs::write(
        opaque_package.join("attack.thpkg.json"),
        r#"{
  "schema": "thermite.package.v1",
  "name": "opaque_static_storage_attack",
  "roots": [
    "api"
  ],
  "modules": [
    {
      "name": "api",
      "path": "api.th",
      "imports": [
        "static_storage"
      ]
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
    let attack_manifest = opaque_package.join("attack.thpkg.json");
    let attack_manifest_s = attack_manifest.to_string_lossy().to_string();
    let attack_out = temp.0.join("opaque-attack.verified");
    let attack_out_s = attack_out.to_string_lossy().to_string();
    let attack = forge(&[
        "build",
        &attack_manifest_s,
        "--level",
        "l3",
        "--export",
        "forge_static_storage_lease",
        "--target",
        "kernel",
        "--out",
        &attack_out_s,
        "--json",
    ]);
    assert!(
        !attack.status.success(),
        "a foreign package module forged an opaque static-storage lease",
    );
    let attack_diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&attack.stdout),
        String::from_utf8_lossy(&attack.stderr),
    );
    assert!(
        attack_diagnostic.contains("constructs `#[opaque]` type `StaticStorageLease`")
            && attack_diagnostic.contains("declared in module `static_storage`"),
        "unexpected opaque-construction diagnostic: {attack_diagnostic}",
    );

    let bundle = temp.0.join("static-storage.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "stdlib/kernel-primitives/static-storage.thpkg.json",
        "--level",
        "l3",
        "--export",
        "static_storage_claim_reason",
        "--crate-name",
        "thermite_static_storage",
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
        "evidence/thermite-package/source/storage/static_storage.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }
    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(
        plan["package"]["name"],
        "thermite_static_storage_primitives"
    );
    assert_eq!(
        plan["exports"][0]["thermite_name"],
        "static_storage_claim_reason",
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
        "strict static-storage surface contains non-faithful TV rows: {tv}",
    );

    let consumer_source = temp.0.join("consumer.rs");
    fs::write(
        &consumer_source,
        r#"fn main() {
    use thermite_static_storage::static_storage_claim_reason;
    assert_eq!(static_storage_claim_reason(false, false, 0, 0), 1);
    assert_eq!(static_storage_claim_reason(true, false, 0, 1), 2);
    assert_eq!(static_storage_claim_reason(false, true, 0, 1), 3);
    assert_eq!(static_storage_claim_reason(false, false, u64::MAX, 1), 4);
    assert_eq!(static_storage_claim_reason(false, false, 0, 1), 0);
}
"#,
    )
    .unwrap();
    let artifact = bundle.join("artifact/libthermite_static_storage.rlib");
    let deps = bundle.join("artifact/deps");
    let consumer = temp.0.join("consumer");
    let compiled = codegen_rustc(&bundle)
        .current_dir(root())
        .arg("--edition=2021")
        .arg(&consumer_source)
        .arg("--extern")
        .arg(format!("thermite_static_storage={}", artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["-C", "panic=abort"])
        .arg("-o")
        .arg(&consumer)
        .output()
        .unwrap();
    assert_success(&compiled);
    assert_success(&Command::new(&consumer).output().unwrap());

    let bound_source = bundle.join("evidence/thermite-package/source/storage/static_storage.th");
    let original = fs::read_to_string(&bound_source).unwrap();
    let weakened = original.replacen(
        "#[opaque] struct StaticStorageLease",
        "struct StaticStorageLease",
        1,
    );
    assert_ne!(
        weakened, original,
        "opaque receipt tamper fixture did not match",
    );
    fs::write(&bound_source, weakened).unwrap();
    let tampered = forge(&["verify-build", &bundle_s, "--replay", "--json"]);
    assert!(
        !tampered.status.success(),
        "removing a receipt-bound opaque barrier unexpectedly replayed",
    );
}
