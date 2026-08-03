//! Receipt-bound construction of the frozen bootable SMP profile.

use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thermite_kernel::{lookup, X86_64_PC_UEFI_SMP_V1};
use thermite_syntax::{Contract, Effect, EffectRow, Item, PrimType, Type};

use crate::cli::ForgeError;
use crate::manifest::Level;

pub const PROFILE: &str = "x86_64-pc-uefi-smp-v1";
static SCRATCH_NONCE: AtomicU64 = AtomicU64::new(0);

fn verus_obligation_function_count(bytes: &[u8]) -> Result<u64, ForgeError> {
    let text = std::str::from_utf8(bytes).map_err(|error| ForgeError::RustcOutput {
        detail: format!("Verus obligation metric input is not UTF-8: {error}"),
    })?;
    Ok(text
        .lines()
        .map(str::trim_start)
        .filter(|line| {
            let mut declaration = *line;
            if let Some(rest) = declaration.strip_prefix("pub ") {
                declaration = rest;
            } else if declaration.starts_with("pub(") {
                let Some((_, rest)) = declaration.split_once(") ") else {
                    return false;
                };
                declaration = rest;
            }
            if let Some(rest) = declaration.strip_prefix("open ") {
                declaration = rest;
            } else if let Some(rest) = declaration.strip_prefix("closed ") {
                declaration = rest;
            }
            [
                "fn ",
                "const fn ",
                "exec fn ",
                "proof fn ",
                "spec fn ",
                "extern \"C\" fn ",
            ]
            .iter()
            .any(|prefix| declaration.starts_with(prefix))
        })
        .count() as u64)
}

pub struct ImageBuildRequest<'a> {
    pub source: &'a Path,
    pub composition_exports: &'a [String],
    pub composition_shells: &'a [PathBuf],
    pub platform: &'a str,
    pub output: &'a Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootEvidence {
    pub cpus: u8,
    pub scenario: String,
    pub transcript_sha256: String,
    pub success_marker: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundBoundary {
    pub name: String,
    pub signature: String,
    pub registry_contract: String,
    pub source_contract_sha256: String,
    pub registry_source_contract_sha256: String,
    pub domain: String,
    pub capability: String,
    pub rights: u32,
    pub symbol: String,
    pub abi: String,
    pub alignment: u16,
    pub ownership: String,
    pub model: String,
    pub concurrency: String,
    pub failure: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelCertificateBinding {
    pub item: String,
    pub level: String,
    pub effects: Vec<String>,
    pub boundary: bool,
    pub boundary_target: Option<String>,
    pub assurance_scope: String,
    pub obligations_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelAuthorshipMetrics {
    pub thermite_loc: u64,
    pub thermite_function_count: u64,
    pub verus_composition_loc: u64,
    pub verus_composition_function_count: u64,
    pub verus_composition_discharged_obligations: u64,
    pub direct_verus_tpl_loc: u64,
    pub direct_verus_tpl_function_count: u64,
    pub direct_verus_discharged_obligations: u64,
    pub rust_assembly_tpl_loc_upper_bound: u64,
    pub ordinary_rust_kernel_logic_loc_upper_bound: u64,
    pub ordinary_rust_kernel_logic_target: u64,
    pub ordinary_rust_kernel_logic_target_met: bool,
    pub declared_platform_boundary_count: u64,
    pub reachable_boundary_count: u64,
    pub reachable_assurance: String,
    pub counting_method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThermiteBootableKernelReceiptV1 {
    pub schema: String,
    pub artifact_class: String,
    pub migration_complete: bool,
    pub profile: String,
    pub assurance_scope: String,
    pub trusted_computing_base: Vec<String>,
    pub source: BoundFile,
    pub thermite_sources: Vec<BoundFile>,
    pub l3_exports: Vec<String>,
    pub boundaries: Vec<BoundBoundary>,
    pub certificates: Vec<KernelCertificateBinding>,
    pub proof_evidence_sha256: String,
    pub registry_sha256: String,
    pub platform_files: Vec<BoundFile>,
    pub composition_shells: Vec<BoundFile>,
    pub verified_policy_binding_sha256: String,
    pub verified_policy_artifact_sha256: String,
    pub verified_policy_files: Vec<BoundFile>,
    pub metrics: KernelAuthorshipMetrics,
    pub toolchain: Vec<String>,
    pub image_path: String,
    pub image_size: u64,
    pub image_sha256: String,
    pub uefi_sha256: String,
    pub debug_symbols_sha256: String,
    pub section_table_sha256: String,
    pub symbol_table_sha256: String,
    pub platform_receipt_sha256: String,
    pub boot_evidence: Vec<BootEvidence>,
    pub reproducible_pair_checked: bool,
    pub binding_sha256: String,
}

pub fn build_image(
    request: ImageBuildRequest<'_>,
) -> Result<ThermiteBootableKernelReceiptV1, ForgeError> {
    if request.platform != PROFILE {
        return Err(ForgeError::Usage(format!(
            "unsupported kernel-image platform `{}`; expected `{PROFILE}`",
            request.platform
        )));
    }
    if request.composition_exports.is_empty() || request.composition_shells.is_empty() {
        return Err(ForgeError::Usage(
            "kernel-image requires at least one `--compose-export` and `--compose-shell`"
                .to_string(),
        ));
    }
    if request.output.extension() != Some(OsStr::new("img")) {
        return Err(ForgeError::Usage(
            "kernel-image output must have the `.img` extension".to_string(),
        ));
    }

    let loaded_input = crate::thermite_package::load(request.source)?;
    let source_bytes = loaded_input.bytes;
    let package = loaded_input.package;
    let source_file_bytes = read(request.source)?;
    let workspace = workspace_root()?;
    let thermite_sources = bind_thermite_sources(&workspace, request.source, package.as_ref())?;
    let parsed = thermite_syntax::parse(std::str::from_utf8(&source_bytes).map_err(|error| {
        ForgeError::RustcOutput {
            detail: format!("kernel source is not UTF-8: {error}"),
        }
    })?);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }
    let boundaries = validate_boundaries(&parsed.program)?;
    if boundaries.is_empty() {
        return Err(ForgeError::RustcOutput {
            detail: "kernel-image proof closure declares no frozen platform boundary".to_string(),
        });
    }
    let boundary_items: Vec<&str> = parsed
        .program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if function.boundary.is_some() => Some(function.name.as_str()),
            _ => None,
        })
        .collect();
    let certificates = crate::check::check_file(request.source)?;
    if certificates.iter().any(|certificate| certificate.slag) {
        return Err(ForgeError::RustcOutput {
            detail: "kernel-image closure contains #[slag]".to_string(),
        });
    }
    for export in request.composition_exports {
        let certificate = certificates
            .iter()
            .find(|certificate| certificate.item == *export)
            .ok_or_else(|| ForgeError::Usage(format!("unknown composition export `{export}`")))?;
        if certificate.level < Level::L3 {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "kernel-image export `{export}` certified at {:?}, not L3 or L4",
                    certificate.level
                ),
            });
        }
    }
    for certificate in &certificates {
        if !boundary_items
            .iter()
            .any(|boundary| *boundary == certificate.item)
            && certificate.level < Level::L3
        {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "reachable kernel item `{}` is below L3 ({:?})",
                    certificate.item, certificate.level
                ),
            });
        }
    }
    let certificate_bindings = bind_certificates(&certificates)?;
    let proof_evidence_sha256 =
        sha256(&serde_json::to_vec(&certificate_bindings).map_err(|error| {
            ForgeError::RustcOutput {
                detail: format!("could not canonicalize kernel proof evidence: {error}"),
            }
        })?);

    let profile_root = workspace.join("platform").join(PROFILE);
    let builder = profile_root.join("build-image.sh");
    let qemu_gate = profile_root.join("test-qemu.py");
    if !builder.is_file() || !qemu_gate.is_file() {
        return Err(ForgeError::RustcOutput {
            detail: "frozen platform builder or QEMU gate is absent".to_string(),
        });
    }

    let output_parent = request.output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_parent).map_err(|source| ForgeError::Io {
        path: output_parent.display().to_string(),
        source,
    })?;
    let scratch = create_scratch(output_parent)?;
    let staged_image = scratch.join("thermite-kernel.img");
    let staged_evidence = scratch.join("boot-evidence");
    let result = (|| {
        let verified_policy_bundle = scratch.join("thermite-kernel-policy.verified");
        let verified_policy = match crate::verified_build::build_composition_file(
            request.source,
            &[],
            request.composition_exports,
            request.composition_shells,
            Some("thermite_kernel_policy"),
            Some(&verified_policy_bundle),
            crate::verified_build::VerifiedTarget::KernelUefi,
        )? {
            crate::verified_build::VerifiedBuildOutcome::Built { receipt, .. } => receipt,
            crate::verified_build::VerifiedBuildOutcome::Rejected { stage, detail } => {
                return Err(ForgeError::RustcOutput {
                    detail: format!(
                        "UEFI Thermite policy composition was rejected at {stage}: {detail}"
                    ),
                });
            }
        };
        let policy_rlib = verified_policy_bundle.join(&verified_policy.binding.artifact.path);
        let policy_deps = verified_policy_bundle.join("artifact/deps");
        run_checked(
            Command::new(&builder)
                .arg(&staged_image)
                .env("THERMITE_VERIFIED_POLICY_RLIB", &policy_rlib)
                .env("THERMITE_VERIFIED_POLICY_DEPS", &policy_deps)
                .current_dir(&workspace),
            "frozen kernel image builder",
        )?;
        run_checked(
            Command::new(&qemu_gate)
                .arg(&staged_image)
                .arg("--output-dir")
                .arg(&staged_evidence)
                .current_dir(&workspace),
            "QEMU/OVMF 1/2/4/8-CPU acceptance matrix",
        )?;

        let staged_efi = scratch.join("thermite-kernel.efi");
        let staged_pdb = scratch.join("thermite-kernel.pdb");
        let staged_sections = scratch.join("thermite-kernel.sections");
        let staged_symbols = scratch.join("thermite-kernel.symbols");
        let staged_platform_receipt = scratch.join("thermite-kernel.receipt");
        let image_bytes = read(&staged_image)?;
        let efi_bytes = read(&staged_efi)?;
        let pdb_bytes = read(&staged_pdb)?;
        let section_bytes = read(&staged_sections)?;
        let symbol_bytes = read(&staged_symbols)?;
        let platform_receipt_bytes = read(&staged_platform_receipt)?;
        let policy_plan = load_verified_policy_plan(&verified_policy_bundle)?;
        validate_direct_refined_symbols(&policy_plan, &symbol_bytes)?;
        if image_bytes.len() != 64 * 1024 * 1024 {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "frozen image has {} bytes, expected 67108864",
                    image_bytes.len()
                ),
            });
        }

        let platform_files = bind_source_allowlist(&workspace, &profile_root)?;
        let verified_policy_files = bind_tree(&verified_policy_bundle, &[])?;
        let metrics = authorship_metrics(
            &workspace,
            &source_bytes,
            &parsed.program,
            request.composition_shells,
            boundaries.len(),
            &verified_policy,
            &verified_policy_bundle,
        )?;
        let mut composition_shells = Vec::new();
        for shell in request.composition_shells {
            let bytes = read(shell)?;
            composition_shells.push(BoundFile {
                path: normalize(shell),
                sha256: sha256(&bytes),
            });
        }
        composition_shells.sort_by(|left, right| left.path.cmp(&right.path));
        let boot_evidence = bind_evidence(&staged_evidence)?;
        let toolchain = vec![
            command_identity("rustc", &["--version"]),
            command_identity("cargo", &["--version"]),
            command_identity("qemu-system-x86_64", &["--version"]),
            command_identity("mkfs.fat", &["--help"]),
        ];
        let registry_sha256 = registry_digest();
        let image_sha256 = sha256(&image_bytes);
        let uefi_sha256 = sha256(&efi_bytes);
        let source = BoundFile {
            path: normalize(request.source),
            sha256: sha256(&source_file_bytes),
        };
        let mut receipt = ThermiteBootableKernelReceiptV1 {
            schema: "ThermitePlatformConformanceReceiptV2".to_string(),
            artifact_class: "platform_conformance_demonstration".to_string(),
            migration_complete: false,
            profile: PROFILE.to_string(),
            assurance_scope: "platform_conformance_to_boundary".to_string(),
            trusted_computing_base: [
                "firmware",
                "hardware",
                "rustc-llvm",
                "linker",
                "target-platform-layer",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            source,
            thermite_sources,
            l3_exports: request.composition_exports.to_vec(),
            boundaries,
            certificates: certificate_bindings,
            proof_evidence_sha256,
            registry_sha256,
            platform_files,
            composition_shells,
            verified_policy_binding_sha256: verified_policy.binding_sha256.clone(),
            verified_policy_artifact_sha256: verified_policy.binding.artifact.sha256.clone(),
            verified_policy_files,
            metrics,
            toolchain,
            image_path: normalize(request.output),
            image_size: image_bytes.len() as u64,
            image_sha256,
            uefi_sha256,
            debug_symbols_sha256: sha256(&pdb_bytes),
            section_table_sha256: sha256(&section_bytes),
            symbol_table_sha256: sha256(&symbol_bytes),
            platform_receipt_sha256: sha256(&platform_receipt_bytes),
            boot_evidence,
            reproducible_pair_checked: true,
            binding_sha256: String::new(),
        };
        receipt.binding_sha256 = receipt_binding(&receipt)?;

        // Rebuild once more before publication and compare both image and EFI.
        let replay_image = scratch.join("thermite-kernel-replay.img");
        run_checked(
            Command::new(&builder)
                .arg(&replay_image)
                .env("THERMITE_VERIFIED_POLICY_RLIB", &policy_rlib)
                .env("THERMITE_VERIFIED_POLICY_DEPS", &policy_deps)
                .current_dir(&workspace),
            "kernel image reproducibility rebuild",
        )?;
        let replay_efi = scratch.join("thermite-kernel-replay.efi");
        let replay_pdb = scratch.join("thermite-kernel-replay.pdb");
        let replay_sections = scratch.join("thermite-kernel-replay.sections");
        let replay_symbols = scratch.join("thermite-kernel-replay.symbols");
        let replay_platform_receipt = scratch.join("thermite-kernel-replay.receipt");
        if read(&replay_image)? != image_bytes
            || read(&replay_efi)? != efi_bytes
            || read(&replay_pdb)? != pdb_bytes
            || read(&replay_sections)? != section_bytes
            || read(&replay_symbols)? != symbol_bytes
            || read(&replay_platform_receipt)? != platform_receipt_bytes
        {
            return Err(ForgeError::RustcOutput {
                detail: "clean kernel-image rebuild was not byte-identical".to_string(),
            });
        }

        publish(
            &receipt,
            &staged_image,
            &staged_evidence,
            &verified_policy_bundle,
            request.output,
        )?;
        Ok(receipt)
    })();
    let cleanup = fs::remove_dir_all(&scratch);
    if let Err(error) = cleanup {
        if result.is_ok() {
            return Err(ForgeError::Io {
                path: scratch.display().to_string(),
                source: error,
            });
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KernelImageValidationReport {
    pub schema: &'static str,
    pub profile: String,
    pub image: String,
    pub image_sha256: String,
    pub binding_sha256: String,
    pub boot_profiles: Vec<u8>,
    pub boot_scenarios: Vec<String>,
    pub replayed: bool,
    pub valid: bool,
}

pub fn validate_image(
    input: &Path,
    replay: bool,
) -> Result<KernelImageValidationReport, ForgeError> {
    let workspace = workspace_root()?;
    let receipt_path = if input.extension() == Some(OsStr::new("img")) {
        let parent = input.parent().unwrap_or_else(|| Path::new("."));
        let stem = input
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| ForgeError::Usage("image path has no UTF-8 stem".to_string()))?;
        parent.join(format!("{stem}.receipt.json"))
    } else {
        input.to_path_buf()
    };
    let receipt_bytes = read(&receipt_path)?;
    let receipt: ThermiteBootableKernelReceiptV1 =
        serde_json::from_slice(&receipt_bytes).map_err(|error| ForgeError::RustcOutput {
            detail: format!("invalid kernel-image receipt JSON: {error}"),
        })?;
    if receipt.schema != "ThermitePlatformConformanceReceiptV2"
        || receipt.artifact_class != "platform_conformance_demonstration"
        || receipt.migration_complete
        || receipt.profile != PROFILE
        || receipt.assurance_scope != "platform_conformance_to_boundary"
    {
        return Err(ForgeError::RustcOutput {
            detail: "kernel-image receipt schema, profile, or assurance scope drifted".to_string(),
        });
    }
    let expected_binding = receipt_binding(&receipt)?;
    if receipt.binding_sha256 != expected_binding {
        return Err(ForgeError::RustcOutput {
            detail: "kernel-image receipt binding digest mismatch".to_string(),
        });
    }
    if receipt.registry_sha256 != registry_digest() {
        return Err(ForgeError::RustcOutput {
            detail: "frozen platform registry differs from the receipt".to_string(),
        });
    }

    let image_path = resolve_workspace_path(&workspace, Path::new(&receipt.image_path));
    let image_bytes = read(&image_path)?;
    if image_bytes.len() as u64 != receipt.image_size
        || sha256(&image_bytes) != receipt.image_sha256
    {
        return Err(ForgeError::RustcOutput {
            detail: "kernel image bytes differ from the receipt".to_string(),
        });
    }
    let image_parent = image_path.parent().unwrap_or_else(|| Path::new("."));
    let image_stem = image_path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| ForgeError::RustcOutput {
            detail: "receipt image path has no UTF-8 stem".to_string(),
        })?;
    let policy_bundle = image_parent.join(format!("{image_stem}.policy"));
    if bind_tree(&policy_bundle, &[])? != receipt.verified_policy_files {
        return Err(ForgeError::RustcOutput {
            detail: "verified Thermite policy bundle differs from the receipt".to_string(),
        });
    }
    let policy_report = crate::verified_build::validate_bundle(&policy_bundle, replay)?;
    let policy_receipt: crate::verified_build::VerifiedBuildReceiptV1 =
        serde_json::from_slice(&read(&policy_bundle.join("receipt.json"))?).map_err(|error| {
            ForgeError::RustcOutput {
                detail: format!("invalid verified Thermite policy receipt: {error}"),
            }
        })?;
    if policy_report.binding_sha256 != receipt.verified_policy_binding_sha256
        || policy_report.artifact_sha256 != receipt.verified_policy_artifact_sha256
    {
        return Err(ForgeError::RustcOutput {
            detail: "verified Thermite policy identity differs from the image receipt".to_string(),
        });
    }
    let policy_plan = load_verified_policy_plan(&policy_bundle)?;
    validate_direct_refined_symbols(
        &policy_plan,
        &read(&image_parent.join(format!("{image_stem}.symbols")))?,
    )?;
    let efi_path = image_parent.join(format!("{image_stem}.efi"));
    if sha256(&read(&efi_path)?) != receipt.uefi_sha256 {
        return Err(ForgeError::RustcOutput {
            detail: "UEFI executable differs from the receipt".to_string(),
        });
    }
    for (suffix, expected, label) in [
        (
            "pdb",
            &receipt.debug_symbols_sha256,
            "debug-symbol artifact",
        ),
        ("sections", &receipt.section_table_sha256, "section table"),
        ("symbols", &receipt.symbol_table_sha256, "symbol table"),
        (
            "receipt",
            &receipt.platform_receipt_sha256,
            "platform build receipt",
        ),
    ] {
        let path = image_parent.join(format!("{image_stem}.{suffix}"));
        if sha256(&read(&path)?) != *expected {
            return Err(ForgeError::RustcOutput {
                detail: format!("{label} differs from the kernel-image receipt"),
            });
        }
    }

    let source_path = resolve_workspace_path(&workspace, Path::new(&receipt.source.path));
    let current_source_file_bytes = read(&source_path)?;
    if sha256(&current_source_file_bytes) != receipt.source.sha256 {
        return Err(ForgeError::RustcOutput {
            detail: "Thermite source differs from the receipt".to_string(),
        });
    }
    let loaded_input = crate::thermite_package::load(&source_path)?;
    let current_source_bytes = loaded_input.bytes;
    if bind_thermite_sources(&workspace, &source_path, loaded_input.package.as_ref())?
        != receipt.thermite_sources
    {
        return Err(ForgeError::RustcOutput {
            detail: "Thermite package source inventory differs from the receipt".to_string(),
        });
    }
    let parsed =
        thermite_syntax::parse(std::str::from_utf8(&current_source_bytes).map_err(|error| {
            ForgeError::RustcOutput {
                detail: format!("Thermite source is not UTF-8: {error}"),
            }
        })?);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }
    let boundaries = validate_boundaries(&parsed.program)?;
    if boundaries != receipt.boundaries {
        return Err(ForgeError::RustcOutput {
            detail: "boundary inventory differs from the receipt".to_string(),
        });
    }

    let profile_root = workspace.join("platform").join(PROFILE);
    if bind_source_allowlist(&workspace, &profile_root)? != receipt.platform_files {
        return Err(ForgeError::RustcOutput {
            detail: "canonical transitive source closure differs from the receipt".to_string(),
        });
    }
    for shell in &receipt.composition_shells {
        let path = resolve_workspace_path(&workspace, Path::new(&shell.path));
        if sha256(&read(&path)?) != shell.sha256 {
            return Err(ForgeError::RustcOutput {
                detail: format!("composition shell differs from receipt: {}", shell.path),
            });
        }
    }
    let shell_paths: Vec<PathBuf> = receipt
        .composition_shells
        .iter()
        .map(|shell| resolve_workspace_path(&workspace, Path::new(&shell.path)))
        .collect();
    let metrics = authorship_metrics(
        &workspace,
        &current_source_bytes,
        &parsed.program,
        &shell_paths,
        boundaries.len(),
        &policy_receipt,
        &policy_bundle,
    )?;
    if metrics != receipt.metrics {
        return Err(ForgeError::RustcOutput {
            detail: "authorship metrics differ from the receipt".to_string(),
        });
    }
    let evidence_path = image_parent.join(format!("{image_stem}.evidence"));
    if bind_evidence(&evidence_path)? != receipt.boot_evidence {
        return Err(ForgeError::RustcOutput {
            detail: "boot evidence differs from the receipt".to_string(),
        });
    }

    let certificates = crate::check::check_file(&source_path)?;
    let certificate_bindings = bind_certificates(&certificates)?;
    if certificate_bindings != receipt.certificates {
        return Err(ForgeError::RustcOutput {
            detail: "kernel proof certificates differ from the receipt".to_string(),
        });
    }
    let proof_digest = sha256(&serde_json::to_vec(&certificate_bindings).map_err(|error| {
        ForgeError::RustcOutput {
            detail: format!("could not canonicalize kernel proof evidence: {error}"),
        }
    })?);
    if proof_digest != receipt.proof_evidence_sha256 {
        return Err(ForgeError::RustcOutput {
            detail: "kernel proof-evidence digest differs from the receipt".to_string(),
        });
    }
    for export in &receipt.l3_exports {
        let level = certificates
            .iter()
            .find(|certificate| certificate.item == *export)
            .map(|certificate| certificate.level)
            .ok_or_else(|| ForgeError::RustcOutput {
                detail: format!("receipt export `{export}` is absent from current source"),
            })?;
        if level < Level::L3 {
            return Err(ForgeError::RustcOutput {
                detail: format!("receipt export `{export}` no longer certifies at L3"),
            });
        }
    }

    if replay {
        let scratch = create_scratch(image_parent)?;
        let replay_image = scratch.join("thermite-kernel.img");
        let replay_evidence = scratch.join("boot-evidence");
        let replay_result = (|| {
            run_checked(
                Command::new(profile_root.join("build-image.sh"))
                    .arg(&replay_image)
                    .env(
                        "THERMITE_VERIFIED_POLICY_RLIB",
                        policy_bundle.join("artifact/libthermite_kernel_policy.rlib"),
                    )
                    .env(
                        "THERMITE_VERIFIED_POLICY_DEPS",
                        policy_bundle.join("artifact/deps"),
                    )
                    .current_dir(&workspace),
                "kernel-image validation rebuild",
            )?;
            if read(&replay_image)? != image_bytes
                || sha256(&read(&scratch.join("thermite-kernel.efi"))?) != receipt.uefi_sha256
                || sha256(&read(&scratch.join("thermite-kernel.pdb"))?)
                    != receipt.debug_symbols_sha256
                || sha256(&read(&scratch.join("thermite-kernel.sections"))?)
                    != receipt.section_table_sha256
                || sha256(&read(&scratch.join("thermite-kernel.symbols"))?)
                    != receipt.symbol_table_sha256
                || sha256(&read(&scratch.join("thermite-kernel.receipt"))?)
                    != receipt.platform_receipt_sha256
            {
                return Err(ForgeError::RustcOutput {
                    detail: "kernel-image replay did not reproduce the published artifacts"
                        .to_string(),
                });
            }
            run_checked(
                Command::new(profile_root.join("test-qemu.py"))
                    .arg(&replay_image)
                    .arg("--output-dir")
                    .arg(&replay_evidence)
                    .current_dir(&workspace),
                "kernel-image validation QEMU replay",
            )?;
            bind_evidence(&replay_evidence)?;
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&scratch);
        replay_result?;
        cleanup.map_err(|source| ForgeError::Io {
            path: scratch.display().to_string(),
            source,
        })?;
    }

    Ok(KernelImageValidationReport {
        schema: "ThermitePlatformConformanceValidationV2",
        profile: receipt.profile,
        image: normalize(&image_path),
        image_sha256: receipt.image_sha256,
        binding_sha256: receipt.binding_sha256,
        boot_profiles: receipt
            .boot_evidence
            .iter()
            .filter(|item| item.scenario == "nominal")
            .map(|item| item.cpus)
            .collect(),
        boot_scenarios: receipt
            .boot_evidence
            .iter()
            .map(|item| item.scenario.clone())
            .collect(),
        replayed: replay,
        valid: true,
    })
}

fn receipt_binding(receipt: &ThermiteBootableKernelReceiptV1) -> Result<String, ForgeError> {
    let mut material = receipt.clone();
    material.binding_sha256.clear();
    let bytes = serde_json::to_vec(&material).map_err(|error| ForgeError::RustcOutput {
        detail: format!("could not canonicalize kernel-image receipt binding: {error}"),
    })?;
    Ok(sha256(&bytes))
}

fn resolve_workspace_path(workspace: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    }
}

fn validate_boundaries(
    program: &thermite_syntax::Program,
) -> Result<Vec<BoundBoundary>, ForgeError> {
    let mut names = Vec::new();
    for item in &program.items {
        let Item::Fn(function) = item else {
            continue;
        };
        let Some(boundary) = &function.boundary else {
            continue;
        };
        let operation = lookup(&boundary.target).map_err(|error| ForgeError::RustcOutput {
            detail: format!(
                "kernel-image boundary `{}` is not an exact frozen registry name: {error:?}",
                boundary.target
            ),
        })?;
        if !operation.source_reachable {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "kernel-image boundary `{}` is implementation-only and cannot be declared by source",
                    boundary.target
                ),
            });
        }
        let signature = format!(
            "fn({})->{}",
            function
                .params
                .iter()
                .map(|parameter| type_spelling(&parameter.ty))
                .collect::<Vec<_>>()
                .join(","),
            type_spelling(&function.ret)
        );
        if signature != operation.signature {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "kernel-image boundary `{}` signature drift: source `{signature}`, \
                     registry `{}`",
                    boundary.target, operation.signature
                ),
            });
        }
        let expected = operation.domain;
        let domain_matches = match &function.contract.fx {
            thermite_syntax::EffectRow::Set(effects) => {
                effects.len() == 1
                    && matches!(
                        effects.first(),
                        Some(Effect::Platform(domain))
                            if domain_name(*domain) == registry_domain_name(expected)
                    )
            }
            thermite_syntax::EffectRow::Pure => false,
        };
        if !domain_matches {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "kernel-image boundary `{}` does not declare platform({})",
                    boundary.target,
                    registry_domain_name(expected)
                ),
            });
        }
        let source_contract_sha256 = source_contract_digest(&function.contract);
        let registry_source_contract_sha256 =
            operation
                .source_contract_sha256
                .ok_or_else(|| ForgeError::RustcOutput {
                    detail: format!(
                        "kernel-image boundary `{}` has no frozen source-contract digest",
                        boundary.target
                    ),
                })?;
        if source_contract_sha256 != registry_source_contract_sha256 {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "kernel-image boundary `{}` contract digest drift: source `{source_contract_sha256}`, registry `{registry_source_contract_sha256}`",
                    boundary.target
                ),
            });
        }
        names.push(BoundBoundary {
            name: boundary.target.clone(),
            signature,
            registry_contract: operation.contract.to_string(),
            source_contract_sha256,
            registry_source_contract_sha256: registry_source_contract_sha256.to_string(),
            domain: registry_domain_name(operation.domain).to_string(),
            capability: format!("{:?}", operation.capability),
            rights: operation.rights.bits(),
            symbol: operation.symbol.to_string(),
            abi: operation.abi.to_string(),
            alignment: operation.alignment,
            ownership: operation.ownership.to_string(),
            model: operation.model.to_string(),
            concurrency: operation.concurrency.to_string(),
            failure: operation.failure.to_string(),
            evidence: operation.evidence.to_string(),
        });
    }
    names.sort_by(|left, right| left.name.cmp(&right.name));
    if names.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return Err(ForgeError::RustcOutput {
            detail: "kernel-image source declares a duplicate frozen boundary".to_string(),
        });
    }
    if let Some(missing) = X86_64_PC_UEFI_SMP_V1.iter().find(|operation| {
        operation.source_reachable
            && !names
                .iter()
                .any(|boundary| boundary.name == operation.name())
    }) {
        return Err(ForgeError::RustcOutput {
            detail: format!(
                "kernel-image source closure is missing reachable frozen boundary `{}`",
                missing.name()
            ),
        });
    }
    Ok(names)
}

/// Hash the semantic contract surface without source-location spans. Receipts
/// must remain stable when an identical boundary declaration moves between
/// receipt-bound package modules, while any requirement, guarantee, bitvector
/// tag, or effect-row change must still alter the identity.
fn source_contract_digest(contract: &Contract) -> String {
    fn field(bytes: &mut Vec<u8>, label: &str, value: &str) {
        bytes.extend_from_slice(label.as_bytes());
        bytes.push(b':');
        bytes.extend_from_slice(value.len().to_string().as_bytes());
        bytes.push(b':');
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(b'\n');
    }

    fn bv_tag(clause: &thermite_syntax::Clause) -> String {
        clause.bv.map_or_else(
            || "none".to_string(),
            |tag| format!("{}:{}", tag.width.spelling(), tag.nowrap),
        )
    }

    let mut bytes = b"ThermiteBoundaryContractV2\n".to_vec();
    field(&mut bytes, "req", &contract.req.text);
    field(&mut bytes, "req-bv", &bv_tag(&contract.req));
    for clause in &contract.ens {
        field(&mut bytes, "ens", &clause.text);
        field(&mut bytes, "ens-bv", &bv_tag(clause));
    }
    match &contract.fx {
        EffectRow::Pure => field(&mut bytes, "fx", "pure"),
        EffectRow::Set(effects) => {
            for effect in effects {
                field(&mut bytes, "fx", &format!("{effect:?}"));
            }
        }
    }
    sha256(&bytes)
}

fn bind_certificates(
    certificates: &[crate::manifest::Certificate],
) -> Result<Vec<KernelCertificateBinding>, ForgeError> {
    let mut bindings = Vec::new();
    for certificate in certificates {
        let obligations = certificate
            .obligations
            .iter()
            .map(|obligation| {
                (
                    obligation.name.as_str(),
                    format!("{:?}", obligation.status),
                    obligation.engine.as_deref(),
                    obligation.trust.as_slice(),
                    obligation
                        .verdict
                        .as_ref()
                        .map(|value| format!("{value:?}")),
                )
            })
            .collect::<Vec<_>>();
        let obligations_sha256 =
            sha256(
                &serde_json::to_vec(&obligations).map_err(|error| ForgeError::RustcOutput {
                    detail: format!("could not canonicalize proof obligations: {error}"),
                })?,
            );
        bindings.push(KernelCertificateBinding {
            item: certificate.item.clone(),
            level: format!("{:?}", certificate.level),
            effects: certificate.effects.clone(),
            boundary: certificate.boundary,
            boundary_target: certificate.boundary_target.clone(),
            assurance_scope: format!("{:?}", certificate.assurance_scope),
            obligations_sha256,
        });
    }
    bindings.sort_by(|left, right| left.item.cmp(&right.item));
    Ok(bindings)
}

fn type_spelling(ty: &Type) -> String {
    match ty {
        Type::Prim(primitive) => match primitive {
            PrimType::U8 => "u8".to_string(),
            PrimType::U16 => "u16".to_string(),
            PrimType::U32 => "u32".to_string(),
            PrimType::U64 => "u64".to_string(),
            PrimType::Usize => "usize".to_string(),
            PrimType::Bool => "bool".to_string(),
        },
        Type::Unit => "()".to_string(),
        Type::Ref { mutable, inner } => {
            format!(
                "&{}{}",
                if *mutable { "mut" } else { "" },
                type_spelling(inner)
            )
        }
        Type::Slice(inner) => format!("[{}]", type_spelling(inner)),
        Type::Generic { name, arg } => format!("{name}<{}>", type_spelling(arg)),
        Type::Named(name) => name.clone(),
        Type::Box(inner) => format!("Box<{}>", type_spelling(inner)),
        Type::Vec(inner) => format!("Vec<{}>", type_spelling(inner)),
        Type::String => "String".to_string(),
        Type::Option(inner) => format!("Option<{}>", type_spelling(inner)),
        Type::Result(ok, error) => {
            format!("Result<{},{}>", type_spelling(ok), type_spelling(error))
        }
        Type::Map(key, value) => {
            format!("Map<{},{}>", type_spelling(key), type_spelling(value))
        }
        Type::Tuple(elements) => format!(
            "({})",
            elements
                .iter()
                .map(type_spelling)
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn domain_name(domain: thermite_syntax::PlatformDomain) -> &'static str {
    domain.surface()
}

fn registry_domain_name(domain: thermite_kernel::PlatformDomain) -> &'static str {
    match domain {
        thermite_kernel::PlatformDomain::Boot => "boot",
        thermite_kernel::PlatformDomain::Memory => "memory",
        thermite_kernel::PlatformDomain::Mmio => "mmio",
        thermite_kernel::PlatformDomain::Pio => "pio",
        thermite_kernel::PlatformDomain::Irq => "irq",
        thermite_kernel::PlatformDomain::Cpu => "cpu",
        thermite_kernel::PlatformDomain::Atomic => "atomic",
        thermite_kernel::PlatformDomain::Smp => "smp",
        thermite_kernel::PlatformDomain::Dma => "dma",
        thermite_kernel::PlatformDomain::Clock => "clock",
        thermite_kernel::PlatformDomain::Entropy => "entropy",
        thermite_kernel::PlatformDomain::Power => "power",
    }
}

fn registry_digest() -> String {
    let mut bytes = Vec::new();
    for entry in X86_64_PC_UEFI_SMP_V1 {
        bytes.extend_from_slice(entry.name().as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(entry.signature.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(entry.contract.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(registry_domain_name(entry.domain).as_bytes());
        bytes.extend_from_slice(format!("{:?}", entry.capability).as_bytes());
        bytes.extend_from_slice(&entry.rights.bits().to_le_bytes());
        bytes.extend_from_slice(entry.symbol.as_bytes());
        bytes.extend_from_slice(entry.source_contract_sha256.unwrap_or("").as_bytes());
        bytes.push(u8::from(entry.source_reachable));
        for domain in entry.secondary_domains {
            bytes.extend_from_slice(registry_domain_name(*domain).as_bytes());
            bytes.push(0);
        }
        bytes.extend_from_slice(entry.abi.as_bytes());
        bytes.extend_from_slice(&entry.alignment.to_le_bytes());
        bytes.extend_from_slice(entry.ownership.as_bytes());
        bytes.extend_from_slice(entry.model.as_bytes());
        bytes.extend_from_slice(entry.concurrency.as_bytes());
        bytes.extend_from_slice(entry.failure.as_bytes());
        bytes.extend_from_slice(entry.evidence.as_bytes());
        bytes.push(0xff);
    }
    sha256(&bytes)
}

fn bind_tree(root: &Path, excluded_components: &[&str]) -> Result<Vec<BoundFile>, ForgeError> {
    fn walk(
        root: &Path,
        path: &Path,
        excluded: &[&str],
        output: &mut Vec<BoundFile>,
    ) -> Result<(), ForgeError> {
        let mut entries = fs::read_dir(path)
            .map_err(|source| ForgeError::Io {
                path: path.display().to_string(),
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| ForgeError::Io {
                path: path.display().to_string(),
                source,
            })?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let entry_path = entry.path();
            if excluded.iter().any(|name| {
                entry_path
                    .components()
                    .any(|part| part.as_os_str() == OsStr::new(name))
            }) {
                continue;
            }
            if entry_path.is_dir() {
                walk(root, &entry_path, excluded, output)?;
            } else if entry_path.is_file() {
                output.push(BoundFile {
                    path: entry_path
                        .strip_prefix(root)
                        .unwrap_or(&entry_path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                    sha256: sha256(&read(&entry_path)?),
                });
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(root, root, excluded_components, &mut files)?;
    Ok(files)
}

fn bind_source_allowlist(
    workspace: &Path,
    profile_root: &Path,
) -> Result<Vec<BoundFile>, ForgeError> {
    let allowlist_path = profile_root.join("source-allowlist.txt");
    let text = fs::read_to_string(&allowlist_path).map_err(|source| ForgeError::Io {
        path: allowlist_path.display().to_string(),
        source,
    })?;
    let mut previous: Option<&str> = None;
    let mut files = Vec::new();
    for entry in text.lines() {
        if entry.is_empty()
            || entry.trim() != entry
            || previous.is_some_and(|last| last >= entry)
            || entry
                .split('/')
                .any(|component| matches!(component, "target" | "dist" | "__pycache__" | ".git"))
        {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "source allowlist must be strictly sorted, canonical, and incidental-free near `{entry}`"
                ),
            });
        }
        let relative = Path::new(entry);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(ForgeError::RustcOutput {
                detail: format!("source allowlist contains unsafe path `{entry}`"),
            });
        }
        let path = workspace.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|source| ForgeError::Io {
            path: path.display().to_string(),
            source,
        })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(ForgeError::RustcOutput {
                detail: format!("source allowlist entry is not a regular file: `{entry}`"),
            });
        }
        files.push(BoundFile {
            path: entry.to_string(),
            sha256: sha256(&read(&path)?),
        });
        previous = Some(entry);
    }
    if files.is_empty() {
        return Err(ForgeError::RustcOutput {
            detail: "source allowlist is empty".to_string(),
        });
    }
    Ok(files)
}

fn bind_thermite_sources(
    workspace: &Path,
    root_source: &Path,
    package: Option<&crate::thermite_package::LoadedPackage>,
) -> Result<Vec<BoundFile>, ForgeError> {
    let absolute_root = resolve_workspace_path(workspace, root_source);
    let root_relative =
        absolute_root
            .strip_prefix(workspace)
            .map_err(|_| ForgeError::RustcOutput {
                detail: "Thermite package root is outside the workspace".to_string(),
            })?;
    let mut files = vec![BoundFile {
        path: normalize(root_relative),
        sha256: sha256(&read(&absolute_root)?),
    }];
    if let Some(package) = package {
        let root = absolute_root.parent().unwrap_or_else(|| Path::new("."));
        for module in &package.modules {
            let absolute_module = root.join(&module.declaration.path);
            let relative_module =
                absolute_module
                    .strip_prefix(workspace)
                    .map_err(|_| ForgeError::RustcOutput {
                        detail: format!(
                            "Thermite package module `{}` is outside the workspace",
                            module.declaration.name
                        ),
                    })?;
            files.push(BoundFile {
                path: normalize(relative_module),
                sha256: sha256(&module.bytes),
            });
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    if files.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(ForgeError::RustcOutput {
            detail: "Thermite package source inventory contains a duplicate path".to_string(),
        });
    }
    Ok(files)
}

fn authorship_metrics(
    workspace: &Path,
    thermite_source: &[u8],
    program: &thermite_syntax::Program,
    shell_paths: &[PathBuf],
    declared_boundary_count: usize,
    verified_policy: &crate::verified_build::VerifiedBuildReceiptV1,
    verified_policy_bundle: &Path,
) -> Result<KernelAuthorshipMetrics, ForgeError> {
    fn source_loc(bytes: &[u8]) -> Result<u64, ForgeError> {
        let text = std::str::from_utf8(bytes).map_err(|error| ForgeError::RustcOutput {
            detail: format!("authorship metric input is not UTF-8: {error}"),
        })?;
        Ok(text.lines().filter(|line| !line.trim().is_empty()).count() as u64)
    }

    let direct_tpl_root = Path::new("platform")
        .join(PROFILE)
        .join("verified")
        .join("tpl");
    let mut verus_composition_loc = 0_u64;
    let mut verus_composition_function_count = 0_u64;
    let mut direct_verus_tpl_loc = 0_u64;
    let mut direct_verus_tpl_function_count = 0_u64;
    for path in shell_paths {
        let bytes = read(path)?;
        let loc = source_loc(&bytes)?;
        let functions = verus_obligation_function_count(&bytes)?;
        let relative = if path.is_absolute() {
            path.strip_prefix(workspace)
                .map_err(|_| ForgeError::RustcOutput {
                    detail: format!(
                        "composition shell is outside the workspace: {}",
                        path.display()
                    ),
                })?
        } else {
            path.as_path()
        };
        if relative.starts_with(&direct_tpl_root) {
            direct_verus_tpl_loc += loc;
            direct_verus_tpl_function_count += functions;
        } else {
            verus_composition_loc += loc;
            verus_composition_function_count += functions;
        }
    }

    let runtime_sources = [
        "platform/x86_64-pc-uefi-smp-v1/runtime/src/main.rs",
        "platform/x86_64-pc-uefi-smp-v1/runtime/src/post_firmware.rs",
    ];
    let mut ordinary_rust_kernel_logic_loc_upper_bound = 0_u64;
    for relative in runtime_sources {
        ordinary_rust_kernel_logic_loc_upper_bound +=
            source_loc(&read(&workspace.join(relative))?)?;
    }
    let rust_assembly_tpl_loc_upper_bound = ordinary_rust_kernel_logic_loc_upper_bound
        + source_loc(&read(
            &workspace.join("platform/x86_64-pc-uefi-smp-v1/runtime/src/ap_trampoline.S"),
        )?)?
        + source_loc(&read(
            &workspace.join("platform/x86_64-pc-uefi-smp-v1/kernel_shell.rs"),
        )?)?;

    if verified_policy.binding.assurance_aggregate.scope != "end_to_end"
        || verified_policy.binding.assurance_aggregate.headline != "L3"
        || verified_policy.binding.target != crate::verified_build::VerifiedTarget::KernelUefi
    {
        return Err(ForgeError::RustcOutput {
            detail: "verified policy metrics require an end-to-end L3 UEFI composition".to_string(),
        });
    }
    let thermite_function_count = program
        .items
        .iter()
        .filter(|item| matches!(item, Item::Fn(function) if function.body.is_some()))
        .count() as u64;
    let verus_evidence: crate::verified_build::VerusEvidence = serde_json::from_slice(&read(
        &verified_policy_bundle.join("evidence/verus-result.json"),
    )?)
    .map_err(|error| ForgeError::RustcOutput {
        detail: format!("invalid whole-crate Verus evidence for metrics: {error}"),
    })?;
    let verus_stdout: serde_json::Value =
        serde_json::from_str(&verus_evidence.stdout).map_err(|error| ForgeError::RustcOutput {
            detail: format!("invalid whole-crate Verus result payload for metrics: {error}"),
        })?;
    let verified_functions = verus_stdout
        .pointer("/verification-results/verified")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| ForgeError::RustcOutput {
            detail: "whole-crate Verus result omitted its verified-function count".to_string(),
        })?;
    // Verus reports one aggregate whole-crate count. Attribute it without the
    // old `verified - source .th functions` shortcut: lowering can synthesize
    // verified language definitions (for example FixedArray8 fill/get/set), and
    // those are neither direct-Verus TPL nor one-for-one source functions. Count
    // the exact receipt-bound lowered definitions, then require the remainder to
    // equal the complete direct-Verus source inventory. Composition proofs and
    // exact TPL refinements are deliberately reported as separate subsets:
    // merely composing generated Thermite is not a proof of a machine
    // operation. Whole-crate success means every member of all three disjoint
    // sets discharged.
    let lowered_thermite_functions = verus_obligation_function_count(&read(
        &verified_policy_bundle.join("evidence/lowered-thermite.verus.rs"),
    )?)?;
    let expected_verified_functions = lowered_thermite_functions
        .checked_add(verus_composition_function_count)
        .and_then(|count| count.checked_add(direct_verus_tpl_function_count))
        .ok_or_else(|| ForgeError::RustcOutput {
            detail: "whole-crate verified-function inventory overflowed".to_string(),
        })?;
    if verified_functions != expected_verified_functions {
        return Err(ForgeError::RustcOutput {
            detail: format!(
                "direct-Verus function inventory disagrees with discharged whole-crate obligations: \
                 Verus reported {verified_functions}, receipt-bound lowered Thermite contributes \
                 {lowered_thermite_functions}, Verus composition declares \
                 {verus_composition_function_count}, and direct-Verus TPL declares \
                 {direct_verus_tpl_function_count}"
            ),
        });
    }
    let artifact_plan = load_verified_policy_plan(verified_policy_bundle)?;
    let directly_refined_boundaries = artifact_plan
        .composition
        .as_ref()
        .map(|composition| composition.direct_refined_boundaries.len() as u64)
        .unwrap_or(0);
    let reachable_assurance = if directly_refined_boundaries == 0 {
        "L3 end_to_end (generated acceptance slice)".to_string()
    } else {
        format!("L3 direct-Verus exact refinement ({directly_refined_boundaries} frozen boundary)")
    };
    let direct_verus_discharged_obligations = direct_verus_tpl_function_count;
    Ok(KernelAuthorshipMetrics {
        thermite_loc: source_loc(thermite_source)?,
        thermite_function_count,
        verus_composition_loc,
        verus_composition_function_count,
        verus_composition_discharged_obligations: verus_composition_function_count,
        direct_verus_tpl_loc,
        direct_verus_tpl_function_count,
        direct_verus_discharged_obligations,
        rust_assembly_tpl_loc_upper_bound,
        ordinary_rust_kernel_logic_loc_upper_bound,
        ordinary_rust_kernel_logic_target: 0,
        ordinary_rust_kernel_logic_target_met: ordinary_rust_kernel_logic_loc_upper_bound == 0,
        declared_platform_boundary_count: declared_boundary_count as u64,
        reachable_boundary_count: directly_refined_boundaries,
        reachable_assurance,
        counting_method: "nonblank physical LOC; Verus composition excludes exact TPL sources under platform/<profile>/verified/tpl; runtime Rust/assembly and ordinary-Rust values are conservative overlapping upper bounds until the remaining platform/policy code is mechanically partitioned"
            .to_string(),
    })
}

fn load_verified_policy_plan(
    verified_policy_bundle: &Path,
) -> Result<crate::verified_build::ArtifactPlanV1, ForgeError> {
    serde_json::from_slice(&read(
        &verified_policy_bundle.join("evidence/artifact-plan.v1"),
    )?)
    .map_err(|error| ForgeError::RustcOutput {
        detail: format!("invalid verified-policy artifact plan: {error}"),
    })
}

fn validate_direct_refined_symbols(
    plan: &crate::verified_build::ArtifactPlanV1,
    symbol_bytes: &[u8],
) -> Result<(), ForgeError> {
    let text = std::str::from_utf8(symbol_bytes).map_err(|error| ForgeError::RustcOutput {
        detail: format!("final-image public-symbol inventory is not UTF-8: {error}"),
    })?;
    if let Some(composition) = &plan.composition {
        for boundary in &composition.direct_refined_boundaries {
            let quoted = format!("`{}`", boundary.implementation_symbol);
            if !text.contains(&quoted) {
                return Err(ForgeError::RustcOutput {
                    detail: format!(
                        "final image omits direct-refinement symbol `{}` for `{}`",
                        boundary.implementation_symbol, boundary.registry_target
                    ),
                });
            }
        }
    }
    Ok(())
}

fn bind_evidence(path: &Path) -> Result<Vec<BootEvidence>, ForgeError> {
    let mut evidence = Vec::new();
    for cpus in [1_u8, 2, 4, 8] {
        let transcript = path.join(format!("boot-{cpus}.log"));
        let bytes = read(&transcript)?;
        require_transcript_marker(
            &bytes,
            b"THERMITE_SUCCESS gate=boot-smp-v1",
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            b"THERMITE_HANDOFF memory_map=1 acpi_bytes=",
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            b"THERMITE_BOUNDARY name=kernel::clock::read@v1 symbol=tpl_clock_read contract=monotonic_with_error resolved=1",
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            b"THERMITE_DEVICE mmio_widths=8,16,32,64 pio_widths=8,16,32 barriers=4 pci=1 virtio=1 negatives=2",
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            format!("THERMITE_CPU_LOCAL installed={cpus} gs_verified={cpus} generation=1")
                .as_bytes(),
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        let policy_flags = 127_u64;
        let signature = policy_flags * 10_000_000 + 1_010_100 + (u64::from(cpus) - 1) * 10 + 41;
        require_transcript_marker(
            &bytes,
            format!(
                "THERMITE_AUTHORED slice=capability+scheduler+ipc+runtime-policy \
                 signature={signature} policy_flags={policy_flags} task_base=41 \
                 applied=generated-dispatch+allocator-mapping-ap-scheduler-shootdown-dma-service-verdicts \
                 functions=receipt assurance=L3+direct-atomic-boundaries migration=partial source=thermite"
            )
            .as_bytes(),
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            b"THERMITE_POST_SCHED tasks=4096 sum=8554496 worker_cpus=",
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            b"THERMITE_ALLOC frames=64 heap_bytes=262144 allocations=3 zeroed=1 reclaimed=1 oom_rejected=1 bridge=global_alloc",
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            b"THERMITE_USER ring=3 syscall_instruction=syscall syscall=1 fault=1 resume=1",
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            format!(
                "THERMITE_ATOMIC increment_total=8386560 message_cpus={cpus} message_stale=0 ordering=release-acquire"
            )
            .as_bytes(),
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        require_transcript_marker(
            &bytes,
            format!(
                "THERMITE_KERNEL mode=freestanding online={cpus} failed=0 failed_apic=4294967295 firmware_calls=0"
            )
            .as_bytes(),
            &format!("{cpus}-CPU nominal transcript"),
        )?;
        evidence.push(BootEvidence {
            cpus,
            scenario: "nominal".to_string(),
            transcript_sha256: sha256(&bytes),
            success_marker: "THERMITE_SUCCESS gate=boot-smp-v1".to_string(),
        });
    }

    let failure_path = path.join("boot-4-failure.log");
    let failure_bytes = read(&failure_path)?;
    for marker in [
        b"THERMITE_AP_FAILURE apic_id=3 state=Failed reason=injected online=3".as_slice(),
        b"THERMITE_KERNEL mode=freestanding online=3 failed=1 failed_apic=3 firmware_calls=0"
            .as_slice(),
        b"THERMITE_SUCCESS gate=boot-smp-v1".as_slice(),
    ] {
        require_transcript_marker(&failure_bytes, marker, "4-CPU AP-start-failure transcript")?;
    }
    evidence.push(BootEvidence {
        cpus: 4,
        scenario: "ap-start-failure".to_string(),
        transcript_sha256: sha256(&failure_bytes),
        success_marker: "THERMITE_SUCCESS gate=boot-smp-v1".to_string(),
    });
    let reboot_path = path.join("boot-2-reboot.log");
    let reboot_bytes = read(&reboot_path)?;
    for marker in [
        b"THERMITE_KERNEL mode=freestanding online=2 failed=0 failed_apic=4294967295 firmware_calls=0"
            .as_slice(),
        b"THERMITE_POWER action=reboot terminal=1".as_slice(),
        b"THERMITE_SUCCESS gate=boot-smp-v1".as_slice(),
    ] {
        require_transcript_marker(&reboot_bytes, marker, "2-CPU reboot transcript")?;
    }
    evidence.push(BootEvidence {
        cpus: 2,
        scenario: "reboot".to_string(),
        transcript_sha256: sha256(&reboot_bytes),
        success_marker: "THERMITE_SUCCESS gate=boot-smp-v1".to_string(),
    });
    Ok(evidence)
}

fn require_transcript_marker(bytes: &[u8], marker: &[u8], label: &str) -> Result<(), ForgeError> {
    if !bytes.windows(marker.len()).any(|window| window == marker) {
        return Err(ForgeError::RustcOutput {
            detail: format!(
                "{label} is missing required marker `{}`",
                String::from_utf8_lossy(marker)
            ),
        });
    }
    Ok(())
}

fn publish(
    receipt: &ThermiteBootableKernelReceiptV1,
    staged_image: &Path,
    staged_evidence: &Path,
    staged_policy: &Path,
    output: &Path,
) -> Result<(), ForgeError> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| ForgeError::Usage("kernel-image output has no UTF-8 stem".to_string()))?;
    let final_efi = parent.join(format!("{stem}.efi"));
    let final_pdb = parent.join(format!("{stem}.pdb"));
    let final_sections = parent.join(format!("{stem}.sections"));
    let final_symbols = parent.join(format!("{stem}.symbols"));
    let final_platform_receipt = parent.join(format!("{stem}.receipt"));
    let final_receipt = parent.join(format!("{stem}.receipt.json"));
    let final_evidence = parent.join(format!("{stem}.evidence"));
    let final_policy = parent.join(format!("{stem}.policy"));
    for path in [
        output,
        &final_efi,
        &final_pdb,
        &final_sections,
        &final_symbols,
        &final_platform_receipt,
        &final_receipt,
        &final_evidence,
        &final_policy,
    ] {
        if path.exists() {
            return Err(ForgeError::Usage(format!(
                "kernel-image publication target already exists: {}",
                path.display()
            )));
        }
    }
    let receipt_bytes =
        serde_json::to_vec_pretty(receipt).map_err(|error| ForgeError::RustcOutput {
            detail: format!("could not encode kernel-image receipt: {error}"),
        })?;
    fs::write(&final_receipt, receipt_bytes).map_err(|source| ForgeError::Io {
        path: final_receipt.display().to_string(),
        source,
    })?;
    let staged_parent = staged_image.parent().unwrap_or_else(|| Path::new("."));
    let staged_stem = staged_image
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| ForgeError::Usage("staged image has no UTF-8 stem".to_string()))?;
    for (suffix, destination) in [
        ("efi", &final_efi),
        ("pdb", &final_pdb),
        ("sections", &final_sections),
        ("symbols", &final_symbols),
        ("receipt", &final_platform_receipt),
    ] {
        let source_path = staged_parent.join(format!("{staged_stem}.{suffix}"));
        fs::copy(&source_path, destination).map_err(|source| ForgeError::Io {
            path: destination.display().to_string(),
            source,
        })?;
    }
    copy_tree(staged_evidence, &final_evidence)?;
    copy_tree_recursive(staged_policy, &final_policy)?;
    // The image is the publication sentinel and is renamed only after proof,
    // reproducibility, receipt, and every boot gate have succeeded.
    fs::rename(staged_image, output).map_err(|source| ForgeError::Io {
        path: output.display().to_string(),
        source,
    })?;
    Ok(())
}

fn copy_tree_recursive(source: &Path, destination: &Path) -> Result<(), ForgeError> {
    fs::create_dir(destination).map_err(|source_error| ForgeError::Io {
        path: destination.display().to_string(),
        source: source_error,
    })?;
    for entry in fs::read_dir(source).map_err(|source_error| ForgeError::Io {
        path: source.display().to_string(),
        source: source_error,
    })? {
        let entry = entry.map_err(|source_error| ForgeError::Io {
            path: source.display().to_string(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|source_error| ForgeError::Io {
                path: destination_path.display().to_string(),
                source: source_error,
            })?;
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), ForgeError> {
    fs::create_dir(destination).map_err(|source_error| ForgeError::Io {
        path: destination.display().to_string(),
        source: source_error,
    })?;
    for entry in fs::read_dir(source).map_err(|source_error| ForgeError::Io {
        path: source.display().to_string(),
        source: source_error,
    })? {
        let entry = entry.map_err(|source_error| ForgeError::Io {
            path: source.display().to_string(),
            source: source_error,
        })?;
        fs::copy(entry.path(), destination.join(entry.file_name())).map_err(|source_error| {
            ForgeError::Io {
                path: destination.display().to_string(),
                source: source_error,
            }
        })?;
    }
    Ok(())
}

fn create_scratch(parent: &Path) -> Result<PathBuf, ForgeError> {
    for _ in 0..32 {
        let nonce = SCRATCH_NONCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".thermite-kernel-image-{}-{nonce}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                return fs::canonicalize(&path).map_err(|source| ForgeError::Io {
                    path: path.display().to_string(),
                    source,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(ForgeError::Io {
                    path: path.display().to_string(),
                    source,
                });
            }
        }
    }
    Err(ForgeError::RustcOutput {
        detail: "could not allocate a unique kernel-image scratch directory".to_string(),
    })
}

fn run_checked(command: &mut Command, stage: &str) -> Result<(), ForgeError> {
    let output = command
        .output()
        .map_err(|source| ForgeError::RustcSpawn { source })?;
    if output.status.success() {
        return Ok(());
    }
    Err(ForgeError::RustcOutput {
        detail: format!(
            "{stage} failed with {:?}: stdout={} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    })
}

fn command_identity(program: &str, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!(
                "{program}:{}{}",
                stdout.lines().next().unwrap_or(""),
                stderr.lines().next().unwrap_or("")
            )
        }
        Err(error) => format!("{program}:unavailable:{error}"),
    }
}

fn workspace_root() -> Result<PathBuf, ForgeError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| ForgeError::RustcOutput {
            detail: "Forge manifest directory has no workspace parent".to_string(),
        })
}

fn read(path: &Path) -> Result<Vec<u8>, ForgeError> {
    fs::read(path).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_receipt() -> ThermiteBootableKernelReceiptV1 {
        ThermiteBootableKernelReceiptV1 {
            schema: "ThermitePlatformConformanceReceiptV2".to_string(),
            artifact_class: "platform_conformance_demonstration".to_string(),
            migration_complete: false,
            profile: PROFILE.to_string(),
            assurance_scope: "platform_conformance_to_boundary".to_string(),
            trusted_computing_base: vec!["hardware".to_string()],
            source: BoundFile {
                path: "kernel.th".to_string(),
                sha256: "source".to_string(),
            },
            thermite_sources: vec![BoundFile {
                path: "kernel.th".to_string(),
                sha256: "source".to_string(),
            }],
            l3_exports: vec!["kernel_step".to_string()],
            boundaries: vec![BoundBoundary {
                name: "kernel::clock::read@v1".to_string(),
                signature: "fn(Clock)->Instant".to_string(),
                registry_contract: "monotonic_with_error".to_string(),
                source_contract_sha256: "contract".to_string(),
                registry_source_contract_sha256: "contract".to_string(),
                domain: "clock".to_string(),
                capability: "Some(Clock)".to_string(),
                rights: 1,
                symbol: "tpl_clock_read".to_string(),
                abi: "C".to_string(),
                alignment: 1,
                ownership: "preserve".to_string(),
                model: "model".to_string(),
                concurrency: "concurrency".to_string(),
                failure: "failure".to_string(),
                evidence: "evidence".to_string(),
            }],
            certificates: vec![KernelCertificateBinding {
                item: "kernel_step".to_string(),
                level: "L3".to_string(),
                effects: vec!["platform(clock)".to_string()],
                boundary: false,
                boundary_target: None,
                assurance_scope: "ToBoundary".to_string(),
                obligations_sha256: "obligations".to_string(),
            }],
            proof_evidence_sha256: "proof".to_string(),
            registry_sha256: "registry".to_string(),
            platform_files: vec![BoundFile {
                path: "runtime.rs".to_string(),
                sha256: "platform".to_string(),
            }],
            composition_shells: vec![BoundFile {
                path: "kernel_shell.rs".to_string(),
                sha256: "shell".to_string(),
            }],
            verified_policy_binding_sha256: "policy-binding".to_string(),
            verified_policy_artifact_sha256: "policy-artifact".to_string(),
            verified_policy_files: vec![BoundFile {
                path: "receipt.json".to_string(),
                sha256: "policy-file".to_string(),
            }],
            metrics: KernelAuthorshipMetrics {
                thermite_loc: 1,
                thermite_function_count: 1,
                verus_composition_loc: 1,
                verus_composition_function_count: 1,
                verus_composition_discharged_obligations: 1,
                direct_verus_tpl_loc: 1,
                direct_verus_tpl_function_count: 1,
                direct_verus_discharged_obligations: 1,
                rust_assembly_tpl_loc_upper_bound: 1,
                ordinary_rust_kernel_logic_loc_upper_bound: 1,
                ordinary_rust_kernel_logic_target: 0,
                ordinary_rust_kernel_logic_target_met: false,
                declared_platform_boundary_count: 1,
                reachable_boundary_count: 0,
                reachable_assurance: "L3 end_to_end".to_string(),
                counting_method: "nonblank".to_string(),
            },
            toolchain: vec!["rustc".to_string()],
            image_path: "kernel.img".to_string(),
            image_size: 1,
            image_sha256: "image".to_string(),
            uefi_sha256: "uefi".to_string(),
            debug_symbols_sha256: "pdb".to_string(),
            section_table_sha256: "sections".to_string(),
            symbol_table_sha256: "symbols".to_string(),
            platform_receipt_sha256: "platform-receipt".to_string(),
            boot_evidence: vec![BootEvidence {
                cpus: 4,
                scenario: "nominal".to_string(),
                transcript_sha256: "transcript".to_string(),
                success_marker: "success".to_string(),
            }],
            reproducible_pair_checked: true,
            binding_sha256: String::new(),
        }
    }

    #[test]
    fn registry_digest_is_stable_and_complete() {
        assert_eq!(registry_digest().len(), 64);
        assert_eq!(
            X86_64_PC_UEFI_SMP_V1.len(),
            thermite_kernel::X86_64_PC_UEFI_SMP_V1_OPERATION_COUNT
        );
        for operation in X86_64_PC_UEFI_SMP_V1
            .iter()
            .filter(|operation| operation.source_reachable)
        {
            assert_eq!(
                thermite_lower::frozen_kernel_boundary_symbol(operation.name()).as_deref(),
                Some(operation.symbol),
                "source boundary symbol drift for {}",
                operation.name()
            );
        }
    }

    #[test]
    fn boundary_inventory_rejects_unknown_and_wrong_domain() {
        let unknown = thermite_syntax::parse(
            "#[boundary(\"kernel::memory::unknown@v1\")] fn b(x: u32) -> u32 req true ens result == x fx platform(memory) ;",
        );
        assert!(unknown.is_clean());
        assert!(validate_boundaries(&unknown.program).is_err());

        let wrong = thermite_syntax::parse(
            "#[boundary(\"kernel::memory::map@v1\")] fn b(x: u32) -> u32 req true ens result == x fx platform(pio) ;",
        );
        assert!(wrong.is_clean());
        assert!(validate_boundaries(&wrong.program).is_err());
    }

    #[test]
    fn boundary_inventory_pins_contract_digest_and_reachable_set() {
        let package_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("conformance/thermite-kernel.thpkg.json");
        let package = crate::thermite_package::load(&package_path).expect("load kernel package");
        let packaged = thermite_syntax::parse(
            std::str::from_utf8(&package.bytes).expect("UTF-8 package program"),
        );
        assert!(packaged.is_clean());
        let packaged_bound =
            validate_boundaries(&packaged.program).expect("packaged frozen boundary");
        assert_eq!(packaged_bound.len(), 9);
        assert!(packaged_bound
            .iter()
            .any(|boundary| boundary.name == "kernel::clock::read@v1"));
        assert!(packaged_bound.iter().all(|boundary| {
            boundary.source_contract_sha256 == boundary.registry_source_contract_sha256
        }));

        let weaker_source = std::str::from_utf8(&package.bytes)
            .expect("UTF-8 package program")
            .replace(
                "ens result.scale_denominator > 0",
                "ens result.scale_denominator >= 0",
            );
        let weaker = thermite_syntax::parse(&weaker_source);
        assert!(weaker.is_clean());
        assert!(validate_boundaries(&weaker.program).is_err());

        let missing = thermite_syntax::parse(
            "fn kernel_step(x: u64) -> u64 req true ens result == x fx pure { x }",
        );
        assert!(missing.is_clean());
        assert!(validate_boundaries(&missing.program).is_err());
    }

    #[test]
    fn receipt_binding_covers_every_proof_implementation_and_boot_closure() {
        let receipt = sample_receipt();
        let baseline = receipt_binding(&receipt).expect("baseline binding");
        macro_rules! changed {
            ($mutation:expr) => {{
                let mut tampered = receipt.clone();
                $mutation(&mut tampered);
                assert_ne!(
                    receipt_binding(&tampered).expect("tampered binding"),
                    baseline
                );
            }};
        }
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.source.sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.thermite_sources[0].sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.boundaries[0]
            .source_contract_sha256
            .push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.certificates[0]
            .obligations_sha256
            .push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.proof_evidence_sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.registry_sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.platform_files[0].sha256.push('x'));
        changed!(
            |r: &mut ThermiteBootableKernelReceiptV1| r.composition_shells[0].sha256.push('x')
        );
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r
            .verified_policy_binding_sha256
            .push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r
            .verified_policy_artifact_sha256
            .push('x'));
        changed!(
            |r: &mut ThermiteBootableKernelReceiptV1| r.verified_policy_files[0].sha256.push('x')
        );
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.metrics.thermite_loc += 1);
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.toolchain[0].push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.image_sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.uefi_sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.debug_symbols_sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.section_table_sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.symbol_table_sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.platform_receipt_sha256.push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.boot_evidence[0]
            .transcript_sha256
            .push('x'));
        changed!(|r: &mut ThermiteBootableKernelReceiptV1| r.reproducible_pair_checked = false);
    }

    #[test]
    fn boot_evidence_rejects_missing_or_mutated_markers() {
        let parent = std::env::temp_dir().join(format!(
            "thermite-kernel-evidence-negative-{}",
            SCRATCH_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&parent).expect("create negative evidence directory");
        fs::write(
            parent.join("boot-1.log"),
            b"THERMITE_SUCCESS gate=boot-smp-v1\n",
        )
        .expect("write truncated evidence");
        assert!(bind_evidence(&parent).is_err());
        fs::remove_dir_all(parent).expect("remove negative evidence directory");
    }

    #[test]
    fn scratch_paths_are_absolute_even_for_relative_output_parents() {
        let parent = PathBuf::from("target").join(format!(
            "thermite-kernel-relative-scratch-{}-{}",
            std::process::id(),
            SCRATCH_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&parent).expect("create relative scratch parent");

        let scratch = create_scratch(&parent).expect("create canonical scratch directory");

        assert!(scratch.is_absolute());
        assert!(scratch
            .starts_with(fs::canonicalize(&parent).expect("canonicalize relative scratch parent")));
        fs::remove_dir_all(parent).expect("remove relative scratch parent");
    }

    #[test]
    fn verus_obligation_inventory_counts_visibility_and_mode_variants() {
        let source = br#"
fn private_exec() {}
pub fn public_exec() {}
pub(crate) extern "C" fn crate_abi() {}
const fn private_const() -> u64 { 0 }
pub exec fn explicit_exec() {}
proof fn private_proof() {}
pub open proof fn public_open_proof() {}
spec fn private_spec() -> bool { true }
open spec fn private_open_spec() -> bool { true }
pub closed spec fn public_closed_spec() -> bool { true }
pub(crate) closed spec fn crate_closed_spec() -> bool { true }
pub struct NotAFunction;
"#;
        assert_eq!(verus_obligation_function_count(source).unwrap(), 11);
    }
}
