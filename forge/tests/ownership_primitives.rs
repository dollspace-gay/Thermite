//! Primitive-only acceptance for the receipt-bound generation discipline.
//!
//! The package contains reusable Thermite state transitions, not a kernel
//! capability policy. `forge check` proves every in-language item at L3 while
//! leaving the one authority-mint boundary honestly at L1. A separate scalar
//! export exercises the strict freestanding build/replay surface until body TV
//! gains named-aggregate and match framing for the complete lifecycle export.

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
            "thermite_ownership_primitives_{name}_{}_{}",
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

#[test]
fn generation_discipline_proves_rejections_and_replays_its_strict_surface() {
    let source = "stdlib/kernel-primitives/ownership/generation.th";
    let temp = TempDir::new("generation");
    let checked = forge(&["check", source, "--level", "l3", "--json"]);
    assert_success(&checked);
    let certificates: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let certificates = certificates.as_array().unwrap();

    let authority = certificates
        .iter()
        .find(|row| row["item"] == "generation_authority")
        .unwrap();
    assert_eq!(authority["level"], "L1");
    assert_eq!(authority["boundary"], true);
    assert_eq!(
        authority["boundary_target"],
        "thermite::ownership::generation_authority"
    );

    for row in certificates {
        if row["item"] == "generation_authority" {
            continue;
        }
        assert_eq!(
            row["level"], "L3",
            "an in-language ownership primitive fell below L3: {row}"
        );
        assert_eq!(
            row["boundary"], false,
            "only the bodyless authority mint may be a boundary: {row}"
        );
    }

    for name in [
        "generation_ledger_init",
        "generation_handle_live",
        "generation_rights_narrow",
        "generation_acquire_at",
        "generation_renew",
        "generation_release",
        "generation_lifecycle_probe",
        "generation_double_release_probe",
        "generation_rights_escalation_probe",
        "generation_slot_reuse_probe",
    ] {
        let row = certificates
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing certificate for `{name}`"));
        assert_eq!(row["level"], "L3", "`{name}` did not prove at L3: {row}");
        assert_ne!(
            row["contract_quality"]["mutants_killed"], "0/0",
            "`{name}` must carry a non-vacuous mutation score"
        );
    }

    let mut adversarial_source = fs::read_to_string(root().join(source)).unwrap();
    adversarial_source.push_str(
        r#"
fn generation_duplicate_ledger_rejected(ledger: GenerationLedger) -> bool
  req !ledger.active[0]
    && ledger.generation[0] < 18_446_744_073_709_551_614 + 1
  ens result
  fx pure
{
  let first: GenerationAcquire = generation_acquire_at(ledger, 0, 1);
  let second: GenerationAcquire = generation_acquire_at(ledger, 0, 1);
  true
}

fn generation_clone_ledger_rejected(ledger: GenerationLedger) -> bool
  req true
  ens result
  fx pure
{
  let duplicate: GenerationLedger = ledger.clone();
  duplicate.authority.identity == ledger.authority.identity
}
"#,
    );
    let adversarial_path = temp.0.join("duplicate-ledger.th");
    fs::write(&adversarial_path, adversarial_source).unwrap();
    let adversarial_path_s = adversarial_path.to_string_lossy().to_string();
    let rejected = forge(&["check", &adversarial_path_s, "--level", "l3", "--json"]);
    assert!(
        !rejected.status.success(),
        "duplicating or cloning the authority-bearing ledger unexpectedly certified"
    );
    let rejected_rows: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    for name in [
        "generation_duplicate_ledger_rejected",
        "generation_clone_ledger_rejected",
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
        opaque_package.join("generation.th"),
        fs::read_to_string(root().join(source)).unwrap(),
    )
    .unwrap();
    fs::write(
        opaque_package.join("api.th"),
        r#"fn forge_generation_handle(
  authority: usize,
  slot: usize,
  generation: u64,
  rights: u64,
) -> GenerationHandle
  req true
  ens result.authority == authority
    && result.slot == slot
    && result.generation == generation
    && result.rights == rights
  fx pure
{
  GenerationHandle {
    authority: authority,
    slot: slot,
    generation: generation,
    rights: rights,
  }
}
"#,
    )
    .unwrap();
    fs::write(
        opaque_package.join("attack.thpkg.json"),
        r#"{
  "schema": "thermite.package.v1",
  "name": "opaque_generation_attack",
  "roots": [
    "api"
  ],
  "modules": [
    {
      "name": "api",
      "path": "api.th",
      "imports": [
        "generation"
      ]
    },
    {
      "name": "generation",
      "path": "generation.th",
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
        "forge_generation_handle",
        "--target",
        "kernel",
        "--out",
        &attack_out_s,
        "--json",
    ]);
    assert!(
        !attack.status.success(),
        "a foreign package module forged an opaque generation handle"
    );
    let attack_diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&attack.stdout),
        String::from_utf8_lossy(&attack.stderr)
    );
    assert!(
        attack_diagnostic.contains("constructs `#[opaque]` type `GenerationHandle`")
            && attack_diagnostic.contains("declared in module `generation`"),
        "unexpected opaque-construction diagnostic: {attack_diagnostic}"
    );

    let bundle = temp.0.join("rights.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "stdlib/kernel-primitives/ownership.thpkg.json",
        "--level",
        "l3",
        "--export",
        "generation_rights_narrow",
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
        "evidence/thermite-package/source/ownership/generation.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }
    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["package"]["name"], "thermite_ownership_primitives");

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
        "strict ownership surface contains non-faithful TV rows: {tv}"
    );

    let bound_source = bundle.join("evidence/thermite-package/source/ownership/generation.th");
    let original = fs::read_to_string(&bound_source).unwrap();
    let weakened = original.replacen(
        "#[opaque] struct GenerationLedger",
        "struct GenerationLedger",
        1,
    );
    assert_ne!(
        weakened, original,
        "opaque receipt tamper fixture did not match"
    );
    fs::write(&bound_source, weakened).unwrap();
    let tampered = forge(&["verify-build", &bundle_s, "--replay", "--json"]);
    assert!(
        !tampered.status.success(),
        "removing a receipt-bound opaque barrier unexpectedly replayed"
    );
}
