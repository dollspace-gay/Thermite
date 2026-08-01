//! Receipt-bound construction of the frozen bootable SMP profile.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thermite_kernel::{lookup, X86_64_PC_UEFI_SMP_V1};
use thermite_syntax::{Effect, Item, PrimType, Type};

use crate::cli::ForgeError;
use crate::manifest::Level;

pub const PROFILE: &str = "x86_64-pc-uefi-smp-v1";
static SCRATCH_NONCE: AtomicU64 = AtomicU64::new(0);

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
pub struct ThermiteBootableKernelReceiptV1 {
    pub schema: String,
    pub profile: String,
    pub assurance_scope: String,
    pub trusted_computing_base: Vec<String>,
    pub source: BoundFile,
    pub l3_exports: Vec<String>,
    pub boundaries: Vec<BoundBoundary>,
    pub certificates: Vec<KernelCertificateBinding>,
    pub proof_evidence_sha256: String,
    pub registry_sha256: String,
    pub platform_files: Vec<BoundFile>,
    pub composition_shells: Vec<BoundFile>,
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

    let source_bytes = read(request.source)?;
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

    let workspace = workspace_root()?;
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
        run_checked(
            Command::new(&builder)
                .arg(&staged_image)
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
        if image_bytes.len() != 64 * 1024 * 1024 {
            return Err(ForgeError::RustcOutput {
                detail: format!(
                    "frozen image has {} bytes, expected 67108864",
                    image_bytes.len()
                ),
            });
        }

        let platform_files = bind_tree(&profile_root, &["target"])?;
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
            sha256: sha256(&source_bytes),
        };
        let mut receipt = ThermiteBootableKernelReceiptV1 {
            schema: "ThermiteBootableKernelReceiptV1".to_string(),
            profile: PROFILE.to_string(),
            assurance_scope: "to_platform_boundary".to_string(),
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
            l3_exports: request.composition_exports.to_vec(),
            boundaries,
            certificates: certificate_bindings,
            proof_evidence_sha256,
            registry_sha256,
            platform_files,
            composition_shells,
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

        publish(&receipt, &staged_image, &staged_evidence, request.output)?;
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
    if receipt.schema != "ThermiteBootableKernelReceiptV1"
        || receipt.profile != PROFILE
        || receipt.assurance_scope != "to_platform_boundary"
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
    if sha256(&read(&source_path)?) != receipt.source.sha256 {
        return Err(ForgeError::RustcOutput {
            detail: "Thermite source differs from the receipt".to_string(),
        });
    }
    let parsed = thermite_syntax::parse(&fs::read_to_string(&source_path).map_err(|source| {
        ForgeError::Io {
            path: source_path.display().to_string(),
            source,
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
    if bind_tree(&profile_root, &["target"])? != receipt.platform_files {
        return Err(ForgeError::RustcOutput {
            detail: "target-platform-layer source closure differs from the receipt".to_string(),
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
        schema: "ThermiteBootableKernelValidationV1",
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
        let source_contract_sha256 = sha256(format!("{:#?}", function.contract).as_bytes());
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
        require_transcript_marker(
            &bytes,
            b"THERMITE_MODEL event_action=1 atomic=1 frame=1 dma_iommu=1 scheduler=1 registry_entries=",
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
    for path in [
        output,
        &final_efi,
        &final_pdb,
        &final_sections,
        &final_symbols,
        &final_platform_receipt,
        &final_receipt,
        &final_evidence,
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
    // The image is the publication sentinel and is renamed only after proof,
    // reproducibility, receipt, and every boot gate have succeeded.
    fs::rename(staged_image, output).map_err(|source| ForgeError::Io {
        path: output.display().to_string(),
        source,
    })?;
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
            Ok(()) => return Ok(path),
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
            schema: "ThermiteBootableKernelReceiptV1".to_string(),
            profile: PROFILE.to_string(),
            assurance_scope: "to_platform_boundary".to_string(),
            trusted_computing_base: vec!["hardware".to_string()],
            source: BoundFile {
                path: "kernel.th".to_string(),
                sha256: "source".to_string(),
            },
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
        let exact = thermite_syntax::parse(include_str!("../../conformance/bootable_kernel.th"));
        assert!(exact.is_clean());
        let bound = validate_boundaries(&exact.program).expect("exact frozen boundary");
        assert_eq!(bound.len(), 1);
        assert_eq!(
            bound[0].source_contract_sha256,
            bound[0].registry_source_contract_sha256
        );

        let weaker_source = include_str!("../../conformance/bootable_kernel.th").replace(
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
}
