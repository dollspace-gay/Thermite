//! Correspondence-backed L3 library builds.
//!
//! This module deliberately does not depend on `crate::build`'s emitter.  The
//! only executable source accepted here is the canonical Verus library emitted
//! by `thermite_lower::lower_l3_library`; that exact file is verified and
//! compiled by one `verus --no-cheating --compile` invocation.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thermite_lower::{L3Export, L3ExportVisibility, L3LibraryTarget};
use thermite_syntax::{
    Block, Effect, EffectRow, Expr, ForgeItem, Item, PrimType, Program, Stmt, Type,
};

use crate::body_tv::{BodyTvReport, BodyVerdict};
use crate::check::{self, DEFAULT_RLIMIT, DEFAULT_SOLVER_SEED};
use crate::cli::ForgeError;
use crate::closure::{self, VerifiedClosure};
use crate::contract_tv::{ClauseVerdict, TvReport};
use crate::exec_tv::{ExecTvReport, ExecVerdict};
use crate::manifest::{AssuranceScope, Certificate, Level, ObligationStatus};

mod composition;

const PLAN_SCHEMA: &str = "thermite.artifact-plan.v1";
const RECEIPT_SCHEMA: &str = "thermite.verified-build-receipt.v1";
const COMPOSITION_PLAN_SCHEMA: &str = "thermite.combined-artifact-plan.v1";
const COMPOSITION_RECEIPT_SCHEMA: &str = "thermite.verified-composition-receipt.v1";
const SOURCE_DATE_EPOCH: &str = "0";
const KERNEL_VSTD_LINK_SOURCE_NAME: &str = "vstd.rs";
const KERNEL_VSTD_LINK_PRELUDE_NAME: &str = "prelude.rs";
const KERNEL_VSTD_LINK_ROOT_SOURCE: &str = include_str!("kernel_vstd_root.rs");
const KERNEL_VSTD_LINK_PRELUDE_SOURCE: &str = include_str!("kernel_vstd_prelude.rs");
const KERNEL_VSTD_SOURCE_EVIDENCE_DIR: &str = "kernel-vstd-source";
const KERNEL_VSTD_LINK_EVIDENCE_DIR: &str = "kernel-vstd-link-source";
const KERNEL_VSTD_VIR_EVIDENCE_NAME: &str = "kernel-vstd.vir";
const KERNEL_BUILTIN_EVIDENCE_NAME: &str = "kernel-verus-builtin/lib.rs";
const KERNEL_VSTD_SOURCE_PATH_KEY: &str = "<kernel-vstd-source-root>";
const KERNEL_VSTD_VIR_PATH_KEY: &str = "<kernel-vstd-vir>";
const KERNEL_BUILTIN_SOURCE_PATH_KEY: &str = "<kernel-verus-builtin-source>";
const STRICT_GATES: &[&str] = &[
    "parse-spec-effects",
    "complete-end-to-end-closure",
    "source-completeness",
    "no-escape-hatches",
    "termination",
    "l3-function-certificates",
    "contract-tv-complete",
    "exec-tv-complete",
    "body-loop-tv-complete",
    "total-export-wrappers",
    "whole-crate-no-cheating",
    "verus-codegen",
    "cryptographic-binding",
];
const COMPOSITION_STRICT_GATES: &[&str] = &[
    "parse-spec-effects",
    "complete-end-to-end-closure",
    "source-completeness",
    "no-escape-hatches",
    "termination",
    "l3-function-certificates",
    "contract-tv-complete",
    "exec-tv-complete",
    "body-loop-tv-complete",
    "total-export-wrappers",
    "rich-composition-visibility",
    "direct-verus-source-policy",
    "combined-source-inventory",
    "whole-crate-no-cheating",
    "verus-codegen",
    "cryptographic-binding",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifiedTarget {
    Std,
    Kernel,
    KernelUefi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedExport {
    pub thermite_name: String,
    pub public_name: String,
    pub source_start: u64,
    pub source_end: u64,
    pub wrapped: bool,
    pub signature: String,
    pub parameter_types: Vec<String>,
    pub ownership: Vec<String>,
    pub return_type: String,
    pub target_triple: String,
    pub target_pointer_width: String,
    pub target_endian: String,
    pub postcondition_ids: Vec<String>,
    pub abi_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedNode {
    pub name: String,
    pub semantic_address: String,
    pub kind: String,
    pub source_start: Option<u64>,
    pub source_end: Option<u64>,
    pub item_sha256: String,
    pub body_sha256: Option<String>,
    pub contract_sha256: Option<String>,
    pub effects_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedTvGate {
    pub phase: String,
    pub label: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedItemDisposition {
    pub name: String,
    pub included: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedCompositionExport {
    pub thermite_name: String,
    pub semantic_address: String,
    pub signature: String,
    pub parameter_types: Vec<String>,
    pub ownership: Vec<String>,
    pub return_type: String,
    pub type_closure: Vec<String>,
    pub visibility: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedShellItem {
    pub name: String,
    pub kind: String,
    pub visibility: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedShellModule {
    pub name: String,
    pub path: String,
    pub source_policy: String,
    pub length: u64,
    pub sha256: String,
    pub items: Vec<PlannedShellItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedDirectRefinedBoundary {
    pub thermite_name: String,
    pub registry_target: String,
    pub implementation_module: String,
    pub implementation_symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionInventoryRow {
    pub origin: String,
    pub name: String,
    pub kind: String,
    pub visibility: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionPlanV1 {
    pub schema: String,
    pub composition_exports: Vec<PlannedCompositionExport>,
    pub shell_modules: Vec<PlannedShellModule>,
    pub direct_refined_boundaries: Vec<PlannedDirectRefinedBoundary>,
    pub inventory: Vec<CompositionInventoryRow>,
    pub lowered_thermite_sha256: String,
    pub combined_source_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThermitePackagePlanV1 {
    pub schema: String,
    pub name: String,
    pub manifest_sha256: String,
    pub modules: Vec<ThermitePackageModulePlanV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThermitePackageModulePlanV1 {
    pub name: String,
    pub path: String,
    pub length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectVerusSource {
    plan: PlannedShellModule,
    bytes: Vec<u8>,
}

fn canonical_shell_set_sha256(modules: &[PlannedShellModule]) -> String {
    let mut c = Canonical::new("thermite.direct-verus-source-set.v1");
    for module in modules {
        c.record("module", |c| {
            c.field("name", &module.name);
            c.field("path", &module.path);
            c.field("source_policy", &module.source_policy);
            c.field("length", &module.length.to_string());
            c.field("sha256", &module.sha256);
            for item in &module.items {
                c.record("item", |c| {
                    c.field("name", &item.name);
                    c.field("kind", &item.kind);
                    c.field("visibility", &item.visibility);
                });
            }
        });
    }
    c.finish()
}

fn canonical_composition_inventory_sha256(rows: &[CompositionInventoryRow]) -> String {
    let mut c = Canonical::new("thermite.composition-inventory.v1");
    for row in rows {
        c.record("item", |c| {
            c.field("origin", &row.origin);
            c.field("name", &row.name);
            c.field("kind", &row.kind);
            c.field("visibility", &row.visibility);
            c.field("sha256", &row.sha256);
        });
    }
    c.finish()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPlanV1 {
    pub schema: String,
    pub raw_source_sha256: String,
    pub parsed_program_sha256: String,
    pub crate_name: String,
    pub target: VerifiedTarget,
    pub target_triple: String,
    pub target_pointer_width: String,
    pub target_endian: String,
    pub crate_type: String,
    pub panic_strategy: String,
    pub expected_verus_args: Vec<String>,
    pub exports: Vec<PlannedExport>,
    pub closure_nodes: Vec<PlannedNode>,
    pub closure_edges: Vec<[String; 2]>,
    pub item_dispositions: Vec<PlannedItemDisposition>,
    pub strict_gates: Vec<String>,
    pub expected_tv_inventory: Vec<PlannedTvGate>,
    pub expected_verus_source_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermite_package: Option<ThermitePackagePlanV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<CompositionPlanV1>,
}

impl ArtifactPlanV1 {
    pub fn canonical_sha256(&self) -> String {
        let mut c = Canonical::new(&self.schema);
        c.field("raw_source_sha256", &self.raw_source_sha256);
        c.field("parsed_program_sha256", &self.parsed_program_sha256);
        c.field("crate_name", &self.crate_name);
        c.field("target", target_name(self.target));
        c.field("target_triple", &self.target_triple);
        c.field("target_pointer_width", &self.target_pointer_width);
        c.field("target_endian", &self.target_endian);
        c.field("crate_type", &self.crate_type);
        c.field("panic_strategy", &self.panic_strategy);
        for arg in &self.expected_verus_args {
            c.field("verus_arg", arg);
        }
        for export in &self.exports {
            c.record("export", |c| {
                c.field("thermite_name", &export.thermite_name);
                c.field("public_name", &export.public_name);
                c.field("source_start", &export.source_start.to_string());
                c.field("source_end", &export.source_end.to_string());
                c.field("wrapped", if export.wrapped { "true" } else { "false" });
                c.field("signature", &export.signature);
                for ty in &export.parameter_types {
                    c.field("parameter_type", ty);
                }
                for ownership in &export.ownership {
                    c.field("ownership", ownership);
                }
                c.field("return_type", &export.return_type);
                c.field("target_triple", &export.target_triple);
                c.field("target_pointer_width", &export.target_pointer_width);
                c.field("target_endian", &export.target_endian);
                for id in &export.postcondition_ids {
                    c.field("postcondition_id", id);
                }
                c.field("abi_sha256", &export.abi_sha256);
            });
        }
        for node in &self.closure_nodes {
            c.record("node", |c| {
                c.field("name", &node.name);
                c.field("semantic_address", &node.semantic_address);
                c.field("kind", &node.kind);
                c.field(
                    "source_start",
                    &node
                        .source_start
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                );
                c.field(
                    "source_end",
                    &node
                        .source_end
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                );
                c.field("item_sha256", &node.item_sha256);
                c.field("body_sha256", node.body_sha256.as_deref().unwrap_or(""));
                c.field(
                    "contract_sha256",
                    node.contract_sha256.as_deref().unwrap_or(""),
                );
                c.field(
                    "effects_sha256",
                    node.effects_sha256.as_deref().unwrap_or(""),
                );
            });
        }
        for edge in &self.closure_edges {
            c.record("edge", |c| {
                c.field("from", &edge[0]);
                c.field("to", &edge[1]);
            });
        }
        for item in &self.item_dispositions {
            c.record("item", |c| {
                c.field("name", &item.name);
                c.field("included", if item.included { "true" } else { "false" });
                c.field("reason", &item.reason);
            });
        }
        for gate in &self.strict_gates {
            c.field("gate", gate);
        }
        for gate in &self.expected_tv_inventory {
            c.record("tv_gate", |c| {
                c.field("phase", &gate.phase);
                c.field("label", &gate.label);
                c.field("count", &gate.count.to_string());
            });
        }
        c.field(
            "expected_verus_source_sha256",
            &self.expected_verus_source_sha256,
        );
        if let Some(package) = &self.thermite_package {
            c.record("thermite_package", |c| {
                c.field("schema", &package.schema);
                c.field("name", &package.name);
                c.field("manifest_sha256", &package.manifest_sha256);
                for module in &package.modules {
                    c.record("module", |c| {
                        c.field("name", &module.name);
                        c.field("path", &module.path);
                        c.field("length", &module.length.to_string());
                        c.field("sha256", &module.sha256);
                    });
                }
            });
        }
        if let Some(composition) = &self.composition {
            c.record("composition", |c| {
                c.field("schema", &composition.schema);
                c.field(
                    "lowered_thermite_sha256",
                    &composition.lowered_thermite_sha256,
                );
                c.field(
                    "combined_source_sha256",
                    &composition.combined_source_sha256,
                );
                for export in &composition.composition_exports {
                    c.record("export", |c| {
                        c.field("thermite_name", &export.thermite_name);
                        c.field("semantic_address", &export.semantic_address);
                        c.field("signature", &export.signature);
                        for ty in &export.parameter_types {
                            c.field("parameter_type", ty);
                        }
                        for ownership in &export.ownership {
                            c.field("ownership", ownership);
                        }
                        c.field("return_type", &export.return_type);
                        for ty in &export.type_closure {
                            c.field("type", ty);
                        }
                        c.field("visibility", &export.visibility);
                    });
                }
                for module in &composition.shell_modules {
                    c.record("shell", |c| {
                        c.field("name", &module.name);
                        c.field("path", &module.path);
                        c.field("source_policy", &module.source_policy);
                        c.field("length", &module.length.to_string());
                        c.field("sha256", &module.sha256);
                        for item in &module.items {
                            c.record("item", |c| {
                                c.field("name", &item.name);
                                c.field("kind", &item.kind);
                                c.field("visibility", &item.visibility);
                            });
                        }
                    });
                }
                for boundary in &composition.direct_refined_boundaries {
                    c.record("direct_refined_boundary", |c| {
                        c.field("thermite_name", &boundary.thermite_name);
                        c.field("registry_target", &boundary.registry_target);
                        c.field("implementation_module", &boundary.implementation_module);
                        c.field("implementation_symbol", &boundary.implementation_symbol);
                    });
                }
                for item in &composition.inventory {
                    c.record("inventory", |c| {
                        c.field("origin", &item.origin);
                        c.field("name", &item.name);
                        c.field("kind", &item.kind);
                        c.field("visibility", &item.visibility);
                        c.field("sha256", &item.sha256);
                    });
                }
            });
        }
        c.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TvEvidenceRow {
    pub phase: String,
    pub label: String,
    pub verdict: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationValidationEvidence {
    pub seed: u64,
    pub rlimit: String,
    pub rows: Vec<TvEvidenceRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerusEvidence {
    pub args: Vec<String>,
    pub source_relative_path: String,
    pub source_sha256_before: String,
    pub source_sha256_after: String,
    pub codegen_toolchain_sha256: String,
    pub success: bool,
    #[serde(default)]
    pub errors: Option<u64>,
    pub stdout: String,
    pub stderr: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<VerusScopeEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerusScopeEvidence {
    pub scope: String,
    pub source_policy: String,
    pub args: Vec<String>,
    pub source_relative_path: String,
    pub source_sha256_before: String,
    pub source_sha256_after: String,
    pub codegen_toolchain_sha256: String,
    pub success: bool,
    #[serde(default)]
    pub errors: Option<u64>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainEvidence {
    pub forge_version: String,
    pub forge_executable_sha256: String,
    pub forge_source_identity: String,
    pub verus_path: String,
    pub verus_sha256: String,
    pub verus_version: String,
    pub rustup_path: String,
    pub rustup_sha256: String,
    pub rustup_version: String,
    pub host_rustc: HostRustcEvidence,
    pub artifact_codegen: CodegenRustcEvidence,
    pub target_triple: String,
    pub target_pointer_width: String,
    pub target_endian: String,
    pub z3_path: String,
    pub z3_sha256: String,
    pub z3_version: String,
    pub cargo_lock_path: String,
    pub cargo_lock_sha256: String,
    pub link_dependencies: Vec<ToolchainDependency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_vstd_model: Option<KernelVstdModelEvidence>,
    pub source_date_epoch: String,
    pub environment: BTreeMap<String, String>,
}

/// Exact proof-model and erased-link identities used by a kernel build.
///
/// `vstd.vir` supplies the already-verified semantics. The full pinned source
/// tree is also compiled for the kernel target: `link_source_sha256` binds its
/// real crate root and `link_rlib_sha256` binds the exact emitted library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelVstdModelEvidence {
    pub target_triple: String,
    pub vir_path: String,
    pub vir_sha256: String,
    pub source_root: String,
    pub source_file_count: u64,
    pub source_total_bytes: u64,
    pub source_sha256: String,
    pub link_source_name: String,
    pub link_source_sha256: String,
    pub link_prelude_sha256: String,
    pub link_build_args: Vec<String>,
    pub link_rlib_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_builtin_source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_builtin_source_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_builtin_build_args: Vec<String>,
}

/// Informational evidence for the Rust compiler selected in Forge's ambient
/// host environment. It is deliberately separate from `artifact_codegen`: the
/// host compiler does not define the ABI of an rlib emitted by Verus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostRustcEvidence {
    pub rustc_path: String,
    pub rustc_sha256: String,
    pub rustc_version: String,
}

/// The Rust/LLVM closure that Verus selects for artifact code generation.
///
/// `rustup_toolchain` comes from the pinned Verus binary's authoritative
/// `Toolchain:` version field. All other identities are resolved through that
/// exact rustup toolchain, never through ambient `rustc` selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodegenRustcEvidence {
    pub selection: String,
    pub rustup_toolchain: String,
    pub rustc_path: String,
    pub rustc_sha256: String,
    pub rustc_version: String,
    pub rustc_release: String,
    pub rustc_commit_hash: String,
    pub sysroot: String,
    pub rustc_component_manifest_path: String,
    pub rustc_component_manifest_sha256: String,
    pub rust_std_component_manifest_path: String,
    pub rust_std_component_manifest_sha256: String,
    pub rustc_driver_path: String,
    pub rustc_driver_sha256: String,
    pub llvm_version: String,
    pub llvm_library_path: String,
    pub llvm_library_sha256: String,
    pub target_triple: String,
    pub target_pointer_width: String,
    pub target_endian: String,
    pub target_libdir: String,
    pub target_libdir_sha256: String,
    pub target_libdir_file_count: u64,
    pub target_libdir_total_bytes: u64,
    pub linker_identity: String,
}

impl CodegenRustcEvidence {
    fn canonical_identity_sha256(&self) -> String {
        let mut c = Canonical::new("thermite.verus-codegen-toolchain.v1");
        c.field("selection", &self.selection);
        c.field("rustup_toolchain", &self.rustup_toolchain);
        c.field("rustc_sha256", &self.rustc_sha256);
        c.field("rustc_version", &self.rustc_version);
        c.field("rustc_release", &self.rustc_release);
        c.field("rustc_commit_hash", &self.rustc_commit_hash);
        c.field(
            "rustc_component_manifest_sha256",
            &self.rustc_component_manifest_sha256,
        );
        c.field(
            "rust_std_component_manifest_sha256",
            &self.rust_std_component_manifest_sha256,
        );
        c.field("rustc_driver_sha256", &self.rustc_driver_sha256);
        c.field("llvm_version", &self.llvm_version);
        c.field("llvm_library_sha256", &self.llvm_library_sha256);
        c.field("target_triple", &self.target_triple);
        c.field("target_pointer_width", &self.target_pointer_width);
        c.field("target_endian", &self.target_endian);
        c.field("target_libdir_sha256", &self.target_libdir_sha256);
        c.field(
            "target_libdir_file_count",
            &self.target_libdir_file_count.to_string(),
        );
        c.field(
            "target_libdir_total_bytes",
            &self.target_libdir_total_bytes.to_string(),
        );
        c.field("linker_identity", &self.linker_identity);
        c.finish()
    }

    fn same_identity(&self, other: &Self) -> bool {
        self.canonical_identity_sha256() == other.canonical_identity_sha256()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainDependency {
    pub name: String,
    pub source_path: String,
    pub sha256: String,
}

struct CollectedToolchain {
    evidence: ToolchainEvidence,
    dependency_paths: BTreeMap<String, PathBuf>,
    _kernel_vstd_scratch: Option<ScratchTree>,
}

impl CollectedToolchain {
    fn dependency_path(&self, name: &str) -> Option<&Path> {
        self.dependency_paths.get(name).map(PathBuf::as_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundFile {
    pub path: String,
    pub length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundArtifact {
    pub path: String,
    pub kind: String,
    pub length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssuranceMember {
    pub name: String,
    pub kind: String,
    pub achieved: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssuranceAggregate {
    pub headline: String,
    pub cap: String,
    pub minimum_reachable: String,
    pub scope: String,
    pub members: Vec<AssuranceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptBindingV1 {
    pub schema: String,
    pub assurance: String,
    pub scope: String,
    pub plan_sha256: String,
    pub raw_source_sha256: String,
    pub parsed_program_sha256: String,
    pub verus_source_sha256: String,
    pub certificate_set_sha256: String,
    pub translation_validation_sha256: String,
    pub whole_crate_verus_sha256: String,
    pub toolchain_sha256: String,
    pub crate_name: String,
    pub target: VerifiedTarget,
    pub artifact: BoundArtifact,
    pub assurance_aggregate: AssuranceAggregate,
    pub exports: Vec<PlannedExport>,
    pub strict_gates: Vec<String>,
    pub files: Vec<BoundFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<CompositionReceiptBindingV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionReceiptBindingV1 {
    pub lowered_thermite_sha256: String,
    pub direct_verus_set_sha256: String,
    pub inventory_sha256: String,
    pub combined_source_sha256: String,
}

impl ReceiptBindingV1 {
    pub fn canonical_sha256(&self) -> String {
        let mut c = Canonical::new(&self.schema);
        c.field("assurance", &self.assurance);
        c.field("scope", &self.scope);
        c.field("plan_sha256", &self.plan_sha256);
        c.field("raw_source_sha256", &self.raw_source_sha256);
        c.field("parsed_program_sha256", &self.parsed_program_sha256);
        c.field("verus_source_sha256", &self.verus_source_sha256);
        c.field("certificate_set_sha256", &self.certificate_set_sha256);
        c.field(
            "translation_validation_sha256",
            &self.translation_validation_sha256,
        );
        c.field("whole_crate_verus_sha256", &self.whole_crate_verus_sha256);
        c.field("toolchain_sha256", &self.toolchain_sha256);
        c.field("crate_name", &self.crate_name);
        c.field("target", target_name(self.target));
        c.record("artifact", |c| {
            c.field("path", &self.artifact.path);
            c.field("kind", &self.artifact.kind);
            c.field("length", &self.artifact.length.to_string());
            c.field("sha256", &self.artifact.sha256);
        });
        c.record("assurance_aggregate", |c| {
            c.field("headline", &self.assurance_aggregate.headline);
            c.field("cap", &self.assurance_aggregate.cap);
            c.field(
                "minimum_reachable",
                &self.assurance_aggregate.minimum_reachable,
            );
            c.field("scope", &self.assurance_aggregate.scope);
            for member in &self.assurance_aggregate.members {
                c.record("member", |c| {
                    c.field("name", &member.name);
                    c.field("kind", &member.kind);
                    c.field("achieved", &member.achieved);
                });
            }
        });
        for export in &self.exports {
            c.record("export", |c| {
                c.field("thermite_name", &export.thermite_name);
                c.field("public_name", &export.public_name);
                c.field("source_start", &export.source_start.to_string());
                c.field("source_end", &export.source_end.to_string());
                c.field("wrapped", if export.wrapped { "true" } else { "false" });
                c.field("signature", &export.signature);
                for ty in &export.parameter_types {
                    c.field("parameter_type", ty);
                }
                for ownership in &export.ownership {
                    c.field("ownership", ownership);
                }
                c.field("return_type", &export.return_type);
                c.field("target_triple", &export.target_triple);
                c.field("target_pointer_width", &export.target_pointer_width);
                c.field("target_endian", &export.target_endian);
                for id in &export.postcondition_ids {
                    c.field("postcondition_id", id);
                }
                c.field("abi_sha256", &export.abi_sha256);
            });
        }
        for gate in &self.strict_gates {
            c.field("gate", gate);
        }
        for file in &self.files {
            c.record("file", |c| {
                c.field("path", &file.path);
                c.field("length", &file.length.to_string());
                c.field("sha256", &file.sha256);
            });
        }
        if let Some(composition) = &self.composition {
            c.record("composition", |c| {
                c.field(
                    "lowered_thermite_sha256",
                    &composition.lowered_thermite_sha256,
                );
                c.field(
                    "direct_verus_set_sha256",
                    &composition.direct_verus_set_sha256,
                );
                c.field("inventory_sha256", &composition.inventory_sha256);
                c.field(
                    "combined_source_sha256",
                    &composition.combined_source_sha256,
                );
            });
        }
        c.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedBuildReceiptV1 {
    pub schema: String,
    pub binding: ReceiptBindingV1,
    pub binding_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedBuildOutcome {
    Built {
        bundle: PathBuf,
        receipt: Box<VerifiedBuildReceiptV1>,
    },
    Rejected {
        stage: String,
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyBuildReport {
    pub bundle: PathBuf,
    pub binding_sha256: String,
    pub replayed: bool,
    pub artifact_sha256: String,
}

struct Canonical {
    bytes: Vec<u8>,
}

impl Canonical {
    fn new(domain: &str) -> Self {
        let mut this = Self { bytes: Vec::new() };
        this.field("domain", domain);
        this
    }

    fn field(&mut self, name: &str, value: &str) {
        self.part(name.as_bytes());
        self.part(value.as_bytes());
    }

    fn record(&mut self, name: &str, f: impl FnOnce(&mut Canonical)) {
        let mut nested = Canonical::new(name);
        f(&mut nested);
        self.part(name.as_bytes());
        self.part(&nested.bytes);
    }

    fn part(&mut self, value: &[u8]) {
        self.bytes
            .extend_from_slice(&(value.len() as u64).to_be_bytes());
        self.bytes.extend_from_slice(value);
    }

    fn finish(self) -> String {
        sha256(&self.bytes)
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Hash the semantic parser product rather than its source-presentation
/// metadata. `Span`, clause/proof `text`, and integer-literal `raw` fields are
/// deliberately omitted: the exact bytes remain independently bound by
/// `raw_source_sha256`, while this digest records the normalized AST consumed by
/// closure planning and lowering.
fn normalized_program_sha256(program: &Program) -> String {
    let debug = format!("{program:#?}");
    sha256(normalize_program_debug(&debug).as_bytes())
}

fn normalize_program_debug(debug: &str) -> String {
    const STRING_FIELDS: [&str; 2] = ["text: \"", "raw: \""];
    let mut normalized = String::with_capacity(debug.len());
    let bytes = debug.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if debug[cursor..].starts_with("Span {") {
            let Some(relative_end) = debug[cursor..].find('}') else {
                normalized.push_str(&debug[cursor..]);
                break;
            };
            normalized.push_str("Span");
            cursor += relative_end + 1;
            continue;
        }
        if let Some(prefix) = STRING_FIELDS
            .iter()
            .find(|prefix| debug[cursor..].starts_with(**prefix))
        {
            normalized.push_str(prefix);
            normalized.push('"');
            cursor += prefix.len();
            let mut escaped = false;
            while cursor < bytes.len() {
                let byte = bytes[cursor];
                cursor += 1;
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    break;
                }
            }
            continue;
        }
        let ch = debug[cursor..]
            .chars()
            .next()
            .expect("cursor is on a UTF-8 boundary");
        normalized.push(ch);
        cursor += ch.len_utf8();
    }
    normalized
}

fn target_name(target: VerifiedTarget) -> &'static str {
    match target {
        VerifiedTarget::Std => "std",
        VerifiedTarget::Kernel => "kernel",
        VerifiedTarget::KernelUefi => "kernel_uefi",
    }
}

fn reject(stage: &str, detail: impl Into<String>) -> VerifiedBuildOutcome {
    VerifiedBuildOutcome::Rejected {
        stage: stage.to_string(),
        detail: detail.into(),
    }
}

/// Construct, prove, compile, bind, self-validate, and atomically publish one
/// correspondence-backed L3 bundle.
pub fn build_file(
    path: &Path,
    exports: &[String],
    crate_name: Option<&str>,
    out: Option<&Path>,
    target: VerifiedTarget,
) -> Result<VerifiedBuildOutcome, ForgeError> {
    let loaded_input = crate::thermite_package::load(path)?;
    let raw_source = loaded_input.bytes;
    let package = loaded_input.package;
    let source_text =
        std::str::from_utf8(&raw_source).map_err(|error| ForgeError::VerusOutput {
            detail: format!("Thermite source is not UTF-8: {error}"),
        })?;
    let parsed = thermite_syntax::parse(source_text);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }
    thermite_spec::validate(&parsed.program).map_err(ForgeError::Spec)?;
    thermite_lower::check_effects(&parsed.program).map_err(ForgeError::Effects)?;

    let crate_name = match crate_name {
        Some(name) if valid_crate_name(name) => name.to_string(),
        Some(name) => {
            return Ok(reject(
                "plan",
                format!("invalid crate name `{name}`; expected [A-Za-z_][A-Za-z0-9_]*"),
            ))
        }
        None => sanitized_crate_name(path),
    };
    if exports.is_empty() {
        return Ok(reject(
            "plan",
            "an L3 build requires at least one explicit export",
        ));
    }
    let destination = match out {
        Some(path) => path.to_path_buf(),
        None => {
            std::env::temp_dir().join(format!("{}.verified.{}", crate_name, std::process::id()))
        }
    };
    if destination.exists() {
        return Ok(reject(
            "publication",
            format!(
                "refusing to overwrite existing bundle `{}`",
                destination.display()
            ),
        ));
    }

    let closure = match closure::verified_closure(&parsed.program, exports) {
        Ok(closure) => closure,
        Err(error) => return Ok(reject("closure", error.to_string())),
    };
    if let Some(detail) = strict_source_checks(&parsed.program, &closure, target, false) {
        return Ok(reject("closure", detail));
    }

    let collected_toolchain = collect_toolchain(target)?;
    let toolchain = &collected_toolchain.evidence;
    let planned_exports = match plan_exports(
        &parsed.program,
        &closure.roots,
        &crate_name,
        target,
        &toolchain.target_triple,
        &toolchain.target_pointer_width,
        &toolchain.target_endian,
    ) {
        Ok(exports) => exports,
        Err(detail) => return Ok(reject("exports", detail)),
    };
    let subprogram = closure_program(&parsed.program, &closure);
    let lowering_exports: Vec<L3Export> = planned_exports
        .iter()
        .map(|export| L3Export {
            source_name: export.thermite_name.clone(),
            public_name: export.public_name.clone(),
            wrapped: export.wrapped,
            visibility: L3ExportVisibility::Public,
        })
        .collect();
    let lower_target = match target {
        VerifiedTarget::Std => L3LibraryTarget::Std,
        VerifiedTarget::Kernel | VerifiedTarget::KernelUefi => L3LibraryTarget::Kernel,
    };
    let verus_source =
        thermite_lower::lower_l3_library(&subprogram, &lowering_exports, lower_target)
            .map_err(ForgeError::Lower)?;
    if let Some(token) = forbidden_emission(&verus_source) {
        return Ok(reject(
            "source-completeness",
            format!("canonical Verus source contains forbidden escape hatch `{token}`"),
        ));
    }

    let plan = make_plan(PlanInput {
        raw_source: &raw_source,
        package: package.as_ref(),
        program: &parsed.program,
        selected_program: &subprogram,
        closure: &closure,
        exports: &planned_exports,
        crate_name: &crate_name,
        target,
        target_triple: &toolchain.target_triple,
        target_pointer_width: &toolchain.target_pointer_width,
        target_endian: &toolchain.target_endian,
        verus_source: &verus_source,
    });
    let frozen_plan_sha = plan.canonical_sha256();

    let mut fresh_source =
        thermite_lower::lower_l3_library(&subprogram, &lowering_exports, lower_target)
            .map_err(ForgeError::Lower)?;
    if test_fault("after-plan-source-mutation") {
        fresh_source.push_str("\n// injected post-plan mutation\n");
    } else if test_fault("after-plan-body-mutation") {
        fresh_source = fresh_source.replacen("\n    x\n", "\n    x + 0\n", 1);
    } else if test_fault("after-plan-helper-mutation") {
        fresh_source = fresh_source.replacen("\nfn helper(", "\nfn helper_changed(", 1);
    } else if test_fault("after-plan-wrapper-mutation") {
        fresh_source = fresh_source.replacen(
            "Err(ThermiteContractError::Precondition)",
            "Err(ThermiteContractError::Precondition) /* changed wrapper */",
            1,
        );
    }
    if sha256(fresh_source.as_bytes()) != plan.expected_verus_source_sha256 {
        return Ok(reject(
            "binding",
            "canonical Verus emission changed after ArtifactPlanV1 was frozen",
        ));
    }

    // Every downstream proof/TV consumer reads a private copy of the exact
    // source bytes that were parsed and plan-bound above. Reopening the caller's
    // path here would permit a filesystem race between planning and the
    // per-item proof passes even though the final Verus source itself is frozen.
    let frozen_input = ScratchTree::new_in_temp(&format!("verified_input_{crate_name}"))?;
    let input_name = if package.is_some() {
        std::ffi::OsStr::new("input.th")
    } else {
        path.file_name()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| std::ffi::OsStr::new("input.th"))
    };
    let frozen_input_path = frozen_input.path.join(input_name);
    write_bytes(&frozen_input_path, &raw_source)?;

    let mut certificates = check::check_file(&frozen_input_path)?;
    inject_certificate_fault(&mut certificates);
    if let Some(detail) = reject_certificates(&certificates, &closure, &parsed.program, false) {
        return Ok(reject("certificates", detail));
    }

    let mut tv = collect_translation_validation(
        &frozen_input_path,
        &parsed.program,
        &closure,
        &planned_exports,
    )?;
    inject_tv_fault(&mut tv);
    if let Some(detail) =
        reject_translation_validation(&tv, &parsed.program, &closure, &planned_exports)
    {
        return Ok(reject("translation-validation", detail));
    }

    if test_fault("before-verus") {
        return Ok(reject("fault-injection", "injected failure before Verus"));
    }
    let compiled = compile_verus_source(
        &crate_name,
        &verus_source,
        target,
        &toolchain.verus_path,
        &toolchain.environment,
        &toolchain.artifact_codegen.canonical_identity_sha256(),
        KernelCompileDependencies {
            vstd_vir: collected_toolchain.dependency_path(KERNEL_VSTD_VIR_PATH_KEY),
            vstd_rlib: collected_toolchain.dependency_path("libvstd.rlib"),
            verus_builtin_rlib: collected_toolchain.dependency_path("libverus_builtin.rlib"),
        },
    )?;
    if !compiled.evidence.success || compiled.evidence.errors != Some(0) {
        return Ok(reject(
            "whole-crate-verus",
            verus_failure_detail("strict Verus proof/codegen failed", &compiled.evidence),
        ));
    }
    if compiled.evidence.source_sha256_before != plan.expected_verus_source_sha256
        || compiled.evidence.source_sha256_after != plan.expected_verus_source_sha256
    {
        return Ok(reject(
            "binding",
            "the final Verus input changed before or during proof/codegen",
        ));
    }
    if test_fault("after-verus") || test_fault("after-codegen") {
        return Ok(reject(
            "fault-injection",
            "injected failure after the exact Verus proof/codegen invocation",
        ));
    }

    let receipt = stage_and_publish(StageInput {
        destination: &destination,
        crate_name: &crate_name,
        target,
        raw_source: &raw_source,
        package: package.as_ref(),
        plan: &plan,
        plan_sha256: &frozen_plan_sha,
        verus_source: &verus_source,
        certificates: &certificates,
        tv: &tv,
        compiled: &compiled,
        toolchain,
        dependency_paths: &collected_toolchain.dependency_paths,
        composition: None,
    })?;

    Ok(VerifiedBuildOutcome::Built {
        bundle: destination,
        receipt: Box::new(receipt),
    })
}

/// Build one exact-source L3 crate containing canonical Thermite lowering and
/// one or more closed direct-Verus shell modules.
pub fn build_composition_file(
    path: &Path,
    link_exports: &[String],
    composition_exports: &[String],
    shell_paths: &[PathBuf],
    crate_name: Option<&str>,
    out: Option<&Path>,
    target: VerifiedTarget,
) -> Result<VerifiedBuildOutcome, ForgeError> {
    composition::build_file(
        path,
        link_exports,
        composition_exports,
        shell_paths,
        crate_name,
        out,
        target,
    )
}

fn valid_crate_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && chars.all(|c| matches!(c, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9'))
}

fn sanitized_crate_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("thermite");
    let mut name: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() || name.as_bytes()[0].is_ascii_digit() {
        name.insert(0, '_');
    }
    name
}

fn closure_program(program: &Program, closure: &VerifiedClosure) -> Program {
    let referrers: Vec<&Item> = program
        .items
        .iter()
        .filter(|item| match item {
            Item::Fn(f) => closure.functions.contains(&f.name),
            Item::SpecFn(s) => closure.spec_functions.contains(&s.name),
            Item::Struct(_) | Item::Enum(_) | Item::Forge(_) => false,
        })
        .collect();
    let adt_names: BTreeSet<String> = crate::check::reachable_adt_deps(program, &referrers)
        .into_iter()
        .map(|item| item.name().to_string())
        .collect();
    Program {
        items: program
            .items
            .iter()
            .filter(|item| match item {
                Item::Fn(f) => closure.functions.contains(&f.name),
                Item::SpecFn(s) => closure.spec_functions.contains(&s.name),
                Item::Struct(s) => adt_names.contains(&s.name),
                Item::Enum(e) => adt_names.contains(&e.name),
                Item::Forge(_) => false,
            })
            .cloned()
            .collect(),
    }
}

fn strict_source_checks(
    program: &Program,
    closure: &VerifiedClosure,
    target: VerifiedTarget,
    direct_frozen_boundaries: bool,
) -> Option<String> {
    for item in &program.items {
        match item {
            Item::Fn(f) if closure.functions.contains(&f.name) => {
                if f.slag.is_some() {
                    return Some(format!(
                        "reachable path `{}` ends at #[slag] function `{}`",
                        closure_path(closure, &f.name).join(" -> "),
                        f.name
                    ));
                }
                let directly_refined_boundary = direct_frozen_boundaries
                    && f.boundary.as_ref().is_some_and(|boundary| {
                        thermite_lower::frozen_kernel_boundary_symbol(&boundary.target).is_some()
                    });
                if f.boundary.is_some() && !directly_refined_boundary {
                    return Some(format!(
                        "reachable path `{}` crosses #[boundary] function `{}`",
                        closure_path(closure, &f.name).join(" -> "),
                        f.name
                    ));
                }
                if f.body.is_none() && f.boundary.is_none() {
                    return Some(format!(
                        "reachable function `{}` has no checked body",
                        f.name
                    ));
                }
                if !f.holes.is_empty() {
                    return Some(format!(
                        "reachable function `{}` contains an open body hole",
                        f.name
                    ));
                }
                let effects: &[Effect] = match &f.contract.fx {
                    EffectRow::Pure => &[],
                    EffectRow::Set(effects) => effects,
                };
                if effects
                    .iter()
                    .any(|effect| matches!(effect, Effect::Diverge))
                {
                    return Some(format!(
                        "reachable path `{}` declares fx diverge at `{}`",
                        closure_path(closure, &f.name).join(" -> "),
                        f.name
                    ));
                }
                if effects.iter().any(|effect| matches!(effect, Effect::Panic)) {
                    return Some(format!("reachable function `{}` declares fx panic", f.name));
                }
                if matches!(target, VerifiedTarget::Kernel | VerifiedTarget::KernelUefi)
                    && effects.iter().any(|effect| {
                        matches!(
                            effect,
                            Effect::Read(_)
                                | Effect::Write(_)
                                | Effect::Net(_)
                                | Effect::Term
                                | Effect::Time
                                | Effect::Rand
                        )
                    })
                {
                    return Some(format!(
                        "reachable kernel function `{}` has an ambient hosted effect",
                        f.name
                    ));
                }
            }
            Item::Forge(forge) if forge_has_holes(forge) => {
                return Some(format!(
                    "proof item `{}` contains an open proof hole",
                    forge.name()
                ));
            }
            _ => {}
        }
    }
    None
}

fn closure_path(closure: &VerifiedClosure, target: &str) -> Vec<String> {
    let mut frontier: Vec<Vec<String>> = closure
        .roots
        .iter()
        .map(|root| vec![root.clone()])
        .collect();
    let mut visited = BTreeSet::new();
    while !frontier.is_empty() {
        frontier.sort();
        let path = frontier.remove(0);
        let Some(last) = path.last() else {
            continue;
        };
        if last == target {
            return path;
        }
        if !visited.insert(last.clone()) {
            continue;
        }
        for (from, to) in &closure.edges {
            if from == last {
                let mut next = path.clone();
                next.push(to.clone());
                frontier.push(next);
            }
        }
    }
    vec![target.to_string()]
}

fn forge_has_holes(item: &ForgeItem) -> bool {
    match item {
        ForgeItem::Lemma(lemma) => !lemma.proof.holes.is_empty(),
        ForgeItem::Proof(proof) => proof
            .obligations
            .iter()
            .any(|obligation| !obligation.proof.holes.is_empty()),
        ForgeItem::PropFn(_) | ForgeItem::Witness(_) => false,
    }
}

fn forbidden_emission(source: &str) -> Option<&'static str> {
    [
        "external_body",
        "assume(",
        "assume ",
        "admit(",
        "decreases *",
        "thermite_check!",
        "lower_l1",
        "no-erasure-check",
    ]
    .into_iter()
    .find(|token| source.contains(token))
}

fn plan_exports(
    program: &Program,
    roots: &[String],
    crate_name: &str,
    target: VerifiedTarget,
    target_triple: &str,
    target_pointer_width: &str,
    target_endian: &str,
) -> Result<Vec<PlannedExport>, String> {
    let mut rows = Vec::new();
    for name in roots {
        let function = program.items.iter().find_map(|item| match item {
            Item::Fn(f) if &f.name == name => Some(f),
            _ => None,
        });
        let Some(function) = function else {
            return Err(format!("unknown executable export `{name}`"));
        };
        if !function.params.iter().all(|p| supported_public_type(&p.ty))
            || !supported_public_type(&function.ret)
        {
            return Err(format!(
                "export `{name}` has a type outside the v1 verified public ABI (primitive scalars and unit only)"
            ));
        }
        let wrapped = !matches!(function.contract.req.expr, Expr::BoolLit(true));
        if wrapped && !executable_precondition(&function.contract.req.expr) {
            return Err(format!(
                "export `{name}` has a non-executable precondition and cannot receive a total wrapper"
            ));
        }
        let public_name = if wrapped {
            format!("thermite_export_{name}_v1")
        } else {
            name.clone()
        };
        let parameter_types = function
            .params
            .iter()
            .map(|p| abi_type(&p.ty))
            .collect::<Vec<_>>();
        let params = function
            .params
            .iter()
            .zip(&parameter_types)
            .map(|(p, ty)| format!("{}:{ty}", p.name))
            .collect::<Vec<_>>()
            .join(",");
        let return_type = if wrapped {
            format!("Result<{},ThermiteContractError>", abi_type(&function.ret))
        } else {
            abi_type(&function.ret)
        };
        let signature = format!("fn {public_name}({params})->{return_type}");
        let ownership = function
            .params
            .iter()
            .map(|_| "by_value".to_string())
            .collect::<Vec<_>>();
        let postcondition_ids = function
            .contract
            .ens
            .iter()
            .enumerate()
            .map(|(index, _)| format!("{}.ens#{}", function.name, index + 1))
            .collect::<Vec<_>>();
        let abi_preimage = format!(
            "thermite-rust-abi-v1\0crate={crate_name}\0profile={}\0triple={target_triple}\0pointer_width={target_pointer_width}\0endian={target_endian}\0ownership={}\0{signature}",
            target_name(target),
            ownership.join(",")
        );
        rows.push(PlannedExport {
            thermite_name: name.clone(),
            public_name,
            source_start: function.span.start as u64,
            source_end: function.span.end() as u64,
            wrapped,
            signature,
            parameter_types,
            ownership,
            return_type,
            target_triple: target_triple.to_string(),
            target_pointer_width: target_pointer_width.to_string(),
            target_endian: target_endian.to_string(),
            postcondition_ids,
            abi_sha256: sha256(abi_preimage.as_bytes()),
        });
    }
    rows.sort_by(|a, b| a.thermite_name.cmp(&b.thermite_name));
    let public: BTreeSet<&str> = rows.iter().map(|row| row.public_name.as_str()).collect();
    if public.len() != rows.len() {
        return Err("two exports map to the same public wrapper name".to_string());
    }
    Ok(rows)
}

fn supported_public_type(ty: &Type) -> bool {
    matches!(ty, Type::Prim(_) | Type::Unit)
}

fn abi_type(ty: &Type) -> String {
    match ty {
        Type::Prim(PrimType::U8) => "u8".to_string(),
        Type::Prim(PrimType::U16) => "u16".to_string(),
        Type::Prim(PrimType::U32) => "u32".to_string(),
        Type::Prim(PrimType::U64) => "u64".to_string(),
        Type::Prim(PrimType::Usize) => "usize".to_string(),
        Type::Prim(PrimType::Bool) => "bool".to_string(),
        Type::Unit => "()".to_string(),
        other => format!("unsupported:{other:?}"),
    }
}

fn executable_precondition(expr: &Expr) -> bool {
    match expr {
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) => true,
        Expr::Binary { lhs, rhs, .. } => {
            executable_precondition(lhs) && executable_precondition(rhs)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => executable_precondition(expr),
        _ => false,
    }
}

struct PlanInput<'a> {
    raw_source: &'a [u8],
    package: Option<&'a crate::thermite_package::LoadedPackage>,
    program: &'a Program,
    selected_program: &'a Program,
    closure: &'a VerifiedClosure,
    exports: &'a [PlannedExport],
    crate_name: &'a str,
    target: VerifiedTarget,
    target_triple: &'a str,
    target_pointer_width: &'a str,
    target_endian: &'a str,
    verus_source: &'a str,
}

fn make_plan(input: PlanInput<'_>) -> ArtifactPlanV1 {
    let PlanInput {
        raw_source,
        package,
        program,
        selected_program,
        closure,
        exports,
        crate_name,
        target,
        target_triple,
        target_pointer_width,
        target_endian,
        verus_source,
    } = input;
    let mut nodes = Vec::new();
    let mut dispositions = Vec::new();
    for item in &program.items {
        let (included, kind) = match item {
            Item::Fn(f) => (closure.functions.contains(&f.name), "fn"),
            Item::SpecFn(s) => (closure.spec_functions.contains(&s.name), "spec_fn"),
            Item::Struct(s) => (
                selected_program
                    .items
                    .iter()
                    .any(|candidate| matches!(candidate, Item::Struct(c) if c.name == s.name)),
                "struct",
            ),
            Item::Enum(e) => (
                selected_program
                    .items
                    .iter()
                    .any(|candidate| matches!(candidate, Item::Enum(c) if c.name == e.name)),
                "enum",
            ),
            Item::Forge(_) => (false, "forge"),
        };
        dispositions.push(PlannedItemDisposition {
            name: item.name().to_string(),
            included,
            reason: if included {
                "reachable-or-required-type".to_string()
            } else {
                "unreachable-or-proof-metadata".to_string()
            },
        });
        if included {
            let PlannedNodeParts {
                source_start,
                source_end,
                body_sha256,
                contract_sha256,
                effects_sha256,
            } = planned_node_parts(item);
            nodes.push(PlannedNode {
                name: item.name().to_string(),
                semantic_address: format!("{kind}::{}", item.name()),
                kind: kind.to_string(),
                source_start,
                source_end,
                item_sha256: sha256(format!("{item:#?}").as_bytes()),
                body_sha256,
                contract_sha256,
                effects_sha256,
            });
        }
    }
    for export in exports.iter().filter(|export| export.wrapped) {
        nodes.push(PlannedNode {
            name: export.public_name.clone(),
            semantic_address: format!("generated::{}", export.public_name),
            kind: "verified_export_wrapper".to_string(),
            source_start: None,
            source_end: None,
            item_sha256: sha256(export.signature.as_bytes()),
            body_sha256: Some(sha256(
                format!("total-result-wrapper:{}", export.thermite_name).as_bytes(),
            )),
            contract_sha256: Some(sha256(
                format!(
                    "requires:true;postconditions:{}",
                    export.postcondition_ids.join(",")
                )
                .as_bytes(),
            )),
            effects_sha256: Some(sha256(b"pure-total-boundary")),
        });
    }
    if exports.iter().any(|export| export.wrapped) {
        nodes.push(PlannedNode {
            name: "ThermiteContractError".to_string(),
            semantic_address: "generated::ThermiteContractError".to_string(),
            kind: "generated_runtime_type".to_string(),
            source_start: None,
            source_end: None,
            item_sha256: sha256(b"pub enum ThermiteContractError { Precondition }"),
            body_sha256: None,
            contract_sha256: None,
            effects_sha256: None,
        });
    }
    nodes.sort_by(|a, b| a.name.cmp(&b.name).then(a.kind.cmp(&b.kind)));
    let edges = closure
        .edges
        .iter()
        .map(|(from, to)| [from.clone(), to.clone()])
        .collect();
    let expected_tv_inventory = expected_tv_inventory(program, closure, exports)
        .into_iter()
        .map(|((phase, label), count)| PlannedTvGate {
            phase,
            label,
            count: count as u64,
        })
        .collect();
    ArtifactPlanV1 {
        schema: PLAN_SCHEMA.to_string(),
        raw_source_sha256: sha256(raw_source),
        parsed_program_sha256: normalized_program_sha256(program),
        crate_name: crate_name.to_string(),
        target,
        target_triple: target_triple.to_string(),
        target_pointer_width: target_pointer_width.to_string(),
        target_endian: target_endian.to_string(),
        crate_type: "rlib".to_string(),
        panic_strategy: "abort".to_string(),
        expected_verus_args: expected_verus_args(crate_name, target),
        exports: exports.to_vec(),
        closure_nodes: nodes,
        closure_edges: edges,
        item_dispositions: dispositions,
        strict_gates: STRICT_GATES
            .iter()
            .map(|gate| (*gate).to_string())
            .collect(),
        expected_tv_inventory,
        expected_verus_source_sha256: sha256(verus_source.as_bytes()),
        thermite_package: package.map(thermite_package_plan),
        composition: None,
    }
}

fn thermite_package_plan(
    package: &crate::thermite_package::LoadedPackage,
) -> ThermitePackagePlanV1 {
    ThermitePackagePlanV1 {
        schema: package.manifest.schema.clone(),
        name: package.manifest.name.clone(),
        manifest_sha256: sha256(&package.manifest_bytes),
        modules: package
            .modules
            .iter()
            .map(|module| ThermitePackageModulePlanV1 {
                name: module.declaration.name.clone(),
                path: module.declaration.path.clone(),
                length: module.bytes.len() as u64,
                sha256: sha256(&module.bytes),
            })
            .collect(),
    }
}

struct PlannedNodeParts {
    source_start: Option<u64>,
    source_end: Option<u64>,
    body_sha256: Option<String>,
    contract_sha256: Option<String>,
    effects_sha256: Option<String>,
}

fn planned_node_parts(item: &Item) -> PlannedNodeParts {
    match item {
        Item::Fn(function) => PlannedNodeParts {
            source_start: Some(function.span.start as u64),
            source_end: Some(function.span.end() as u64),
            body_sha256: function
                .body
                .as_ref()
                .map(|body| sha256(format!("{body:#?}").as_bytes())),
            contract_sha256: Some(sha256(
                format!("{:#?}:{:#?}", function.contract, function.dec).as_bytes(),
            )),
            effects_sha256: Some(sha256(format!("{:#?}", function.contract.fx).as_bytes())),
        },
        Item::SpecFn(function) => PlannedNodeParts {
            source_start: Some(function.span.start as u64),
            source_end: Some(function.span.end() as u64),
            body_sha256: Some(sha256(format!("{:#?}", function.body).as_bytes())),
            contract_sha256: Some(sha256(format!("{:#?}", function.dec).as_bytes())),
            effects_sha256: Some(sha256(b"spec-pure")),
        },
        Item::Struct(item) => PlannedNodeParts {
            source_start: Some(item.span.start as u64),
            source_end: Some(item.span.end() as u64),
            body_sha256: None,
            contract_sha256: item
                .inv
                .as_ref()
                .map(|inv| sha256(format!("{inv:#?}").as_bytes())),
            effects_sha256: None,
        },
        Item::Enum(item) => PlannedNodeParts {
            source_start: Some(item.span.start as u64),
            source_end: Some(item.span.end() as u64),
            body_sha256: None,
            contract_sha256: None,
            effects_sha256: None,
        },
        Item::Forge(_) => PlannedNodeParts {
            source_start: None,
            source_end: None,
            body_sha256: None,
            contract_sha256: None,
            effects_sha256: None,
        },
    }
}

fn reject_certificates(
    certs: &[Certificate],
    closure: &VerifiedClosure,
    program: &Program,
    direct_frozen_boundaries: bool,
) -> Option<String> {
    let required: BTreeSet<&str> = closure
        .functions
        .iter()
        .chain(closure.spec_functions.iter())
        .map(String::as_str)
        .collect();
    for name in required {
        let source_range = program
            .items
            .iter()
            .find(|item| item.name() == name)
            .and_then(|item| match item {
                Item::Fn(item) => Some((item.span.start, item.span.end())),
                Item::SpecFn(item) => Some((item.span.start, item.span.end())),
                Item::Struct(item) => Some((item.span.start, item.span.end())),
                Item::Enum(item) => Some((item.span.start, item.span.end())),
                Item::Forge(_) => None,
            })
            .map(|(start, end)| format!(" (Thermite bytes {start}..{end})"))
            .unwrap_or_default();
        let Some(cert) = certs.iter().find(|cert| cert.item == name) else {
            return Some(format!(
                "missing certificate for reachable node `{name}`{source_range}"
            ));
        };
        let directly_refined_boundary =
            direct_frozen_boundaries && is_frozen_boundary_function(program, name);
        if directly_refined_boundary {
            if cert.level < Level::L1
                || !cert.boundary
                || cert.slag
                || cert.reject.is_some()
                || cert.lowered_assurance
                || cert
                    .obligations
                    .iter()
                    .any(|obligation| obligation.status != ObligationStatus::Discharged)
            {
                return Some(format!(
                    "reachable frozen boundary `{name}` lacks valid L1 declaration evidence"
                ));
            }
            continue;
        }
        if cert.level < Level::L3 {
            let proof_diagnostic = cert
                .obligations
                .iter()
                .find(|obligation| obligation.status == ObligationStatus::Failed)
                .map(|obligation| {
                    format!(
                        " at {}: {}",
                        obligation
                            .location
                            .as_deref()
                            .unwrap_or("unknown source location"),
                        obligation
                            .diagnostic
                            .as_deref()
                            .unwrap_or("proof obligation failed")
                    )
                })
                .unwrap_or_default();
            return Some(format!(
                "reachable node `{name}` achieved {:?}, not L3{source_range}{proof_diagnostic}",
                cert.level
            ));
        }
        if cert.lowered_assurance || cert.reject.is_some() || cert.slag || cert.boundary {
            return Some(format!(
                "reachable node `{name}` has degraded, rejected, slag, or boundary evidence"
            ));
        }
        let scope_is_directly_refined = direct_frozen_boundaries
            && matches!(
                &cert.assurance_scope,
                Some(AssuranceScope::ToBoundary { via })
                    if is_frozen_boundary_function(program, via)
            );
        if !matches!(cert.assurance_scope, None | Some(AssuranceScope::EndToEnd))
            && !scope_is_directly_refined
        {
            return Some(format!("reachable node `{name}` is not end-to-end"));
        }
        if cert
            .obligations
            .iter()
            .any(|obligation| obligation.status != ObligationStatus::Discharged)
        {
            return Some(format!("reachable node `{name}` has a failed obligation"));
        }
    }
    None
}

fn is_frozen_boundary_function(program: &Program, name: &str) -> bool {
    program.items.iter().any(|item| match item {
        Item::Fn(function) if function.name == name => {
            function.boundary.as_ref().is_some_and(|boundary| {
                thermite_lower::frozen_kernel_boundary_symbol(&boundary.target).is_some()
            })
        }
        _ => false,
    })
}

fn inject_certificate_fault(certificates: &mut Vec<Certificate>) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Ok(fault) = std::env::var("THERMITE_L3_TEST_FAULT") else {
        return;
    };
    match fault.as_str() {
        "certificate-missing" => {
            certificates.clear();
        }
        "certificate-l1" => {
            for certificate in certificates.iter_mut() {
                certificate.level = Level::L1;
            }
        }
        "certificate-l2" => {
            for certificate in certificates.iter_mut() {
                certificate.level = Level::L2;
            }
        }
        "certificate-timeout" => {
            for certificate in certificates.iter_mut() {
                certificate.lowered_assurance = true;
            }
        }
        "certificate-counterexample" => {
            for certificate in certificates.iter_mut() {
                certificate.level = Level::L0;
            }
        }
        "certificate-rejected" => {
            for certificate in certificates.iter_mut() {
                certificate.reject = Some(crate::manifest::RejectReason {
                    cause: "InjectedReject".to_string(),
                    detail: "controlled rejected certificate".to_string(),
                });
            }
        }
        "certificate-failed-obligation" => {
            for certificate in certificates.iter_mut() {
                if let Some(obligation) = certificate.obligations.first_mut() {
                    obligation.status = ObligationStatus::Failed;
                    obligation.diagnostic = Some("controlled failed obligation".to_string());
                }
            }
        }
        _ => {}
    }
}

fn assurance_aggregate(
    certificates: &[Certificate],
    closure: &VerifiedClosure,
    exports: &[PlannedExport],
    direct_refined_boundaries: &BTreeSet<String>,
) -> Result<AssuranceAggregate, ForgeError> {
    let required: BTreeSet<&str> = closure
        .functions
        .iter()
        .chain(closure.spec_functions.iter())
        .map(String::as_str)
        .collect();
    let mut minimum = Level::L4;
    let mut members = Vec::new();
    for name in required {
        let certificate = certificates
            .iter()
            .find(|certificate| certificate.item == name)
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: format!("missing certificate while aggregating `{name}`"),
            })?;
        let directly_refined = direct_refined_boundaries.contains(name);
        if directly_refined && !certificate.boundary {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "direct-refinement inventory names non-boundary certificate `{name}`"
                ),
            });
        }
        let effective_level = if directly_refined {
            Level::L3
        } else {
            certificate.level
        };
        minimum = minimum.min(effective_level);
        members.push(AssuranceMember {
            name: name.to_string(),
            kind: if directly_refined {
                "frozen_boundary_direct_verus".to_string()
            } else if closure.functions.contains(name) {
                "executable".to_string()
            } else {
                "specification".to_string()
            },
            achieved: if directly_refined {
                "L3_direct_refinement".to_string()
            } else {
                level_name(certificate.level).to_string()
            },
        });
    }
    for export in exports.iter().filter(|export| export.wrapped) {
        minimum = minimum.min(Level::L3);
        members.push(AssuranceMember {
            name: export.public_name.clone(),
            kind: "generated_export_wrapper".to_string(),
            achieved: "L3".to_string(),
        });
    }
    members.sort_by(|left, right| left.name.cmp(&right.name).then(left.kind.cmp(&right.kind)));
    let minimum_reachable = level_name(minimum).to_string();
    if minimum < Level::L3 {
        return Err(ForgeError::VerusOutput {
            detail: format!("verified artifact aggregate fell below L3 at {minimum_reachable}"),
        });
    }
    Ok(AssuranceAggregate {
        headline: "L3".to_string(),
        cap: "L3".to_string(),
        minimum_reachable,
        scope: "end_to_end".to_string(),
        members,
    })
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::L0 => "L0",
        Level::L1 => "L1",
        Level::L2 => "L2",
        Level::L3 => "L3",
        Level::L4 => "L4",
    }
}

fn collect_translation_validation(
    path: &Path,
    program: &Program,
    closure: &VerifiedClosure,
    exports: &[PlannedExport],
) -> Result<TranslationValidationEvidence, ForgeError> {
    let contract = crate::contract_tv::tv_file(path, DEFAULT_SOLVER_SEED, DEFAULT_RLIMIT)?;
    let exec = crate::exec_tv::exec_tv_file(path, DEFAULT_SOLVER_SEED, DEFAULT_RLIMIT)?;
    let body = crate::body_tv::body_tv_file(path, DEFAULT_SOLVER_SEED, DEFAULT_RLIMIT)?;
    let mut rows = Vec::new();
    append_contract_rows(&mut rows, contract, closure);
    append_exec_rows(&mut rows, exec, closure);
    append_body_rows(&mut rows, body, closure);
    for export in exports.iter().filter(|export| export.wrapped) {
        let Some(function) = program.items.iter().find_map(|item| match item {
            Item::Fn(f) if f.name == export.thermite_name => Some(f),
            _ => None,
        }) else {
            continue;
        };
        let result =
            crate::exec_tv::exec_tv_export_guard(function, DEFAULT_SOLVER_SEED, DEFAULT_RLIMIT);
        let (verdict, detail) = match result.verdict {
            ExecVerdict::Faithful => ("faithful", None),
            ExecVerdict::Divergent { detail } => ("divergent", Some(detail)),
            ExecVerdict::Unverifiable { reason } => ("unverifiable", Some(reason)),
            ExecVerdict::Skipped { reason } => ("skipped", Some(reason)),
        };
        rows.push(TvEvidenceRow {
            phase: "wrapper_guard".to_string(),
            label: result.label,
            verdict: verdict.to_string(),
            detail,
        });
    }
    rows.sort_by(|a, b| a.phase.cmp(&b.phase).then(a.label.cmp(&b.label)));
    Ok(TranslationValidationEvidence {
        seed: DEFAULT_SOLVER_SEED,
        rlimit: DEFAULT_RLIMIT.to_string(),
        rows,
    })
}

fn reachable_label(label: &str, closure: &VerifiedClosure) -> bool {
    closure.functions.iter().any(|name| {
        label == name
            || label
                .strip_prefix(name)
                .is_some_and(|rest| rest.starts_with('.'))
    })
}

fn append_contract_rows(
    rows: &mut Vec<TvEvidenceRow>,
    report: TvReport,
    closure: &VerifiedClosure,
) {
    for result in report.clauses {
        if !reachable_label(&result.label, closure) {
            continue;
        }
        let (verdict, detail) = match result.verdict {
            ClauseVerdict::Faithful => ("faithful", None),
            ClauseVerdict::Divergent { detail } => ("divergent", Some(detail)),
            ClauseVerdict::Skipped { reason } => ("skipped", Some(reason)),
            ClauseVerdict::Unverifiable => ("unverifiable", None),
        };
        rows.push(TvEvidenceRow {
            phase: "contract".to_string(),
            label: result.label,
            verdict: verdict.to_string(),
            detail,
        });
    }
}

fn append_exec_rows(
    rows: &mut Vec<TvEvidenceRow>,
    report: ExecTvReport,
    closure: &VerifiedClosure,
) {
    for result in report.results {
        if !reachable_label(&result.label, closure) {
            continue;
        }
        if result.label.ends_with(".loop")
            || result.label.ends_with(".if")
            || result.label.ends_with(".assign")
        {
            // These are statement-class handoff markers, not executable
            // expressions. The complete body/loop TV phase below owns them.
            continue;
        }
        let (verdict, detail) = match result.verdict {
            ExecVerdict::Faithful => ("faithful", None),
            ExecVerdict::Divergent { detail } => ("divergent", Some(detail)),
            ExecVerdict::Unverifiable { reason } => ("unverifiable", Some(reason)),
            ExecVerdict::Skipped { reason } => ("skipped", Some(reason)),
        };
        rows.push(TvEvidenceRow {
            phase: "exec".to_string(),
            label: result.label,
            verdict: verdict.to_string(),
            detail,
        });
    }
}

fn append_body_rows(
    rows: &mut Vec<TvEvidenceRow>,
    report: BodyTvReport,
    closure: &VerifiedClosure,
) {
    for result in report.results {
        if !reachable_label(&result.label, closure) {
            continue;
        }
        let (verdict, detail) = match result.verdict {
            BodyVerdict::Faithful => ("faithful", None),
            BodyVerdict::Divergent { detail } => ("divergent", Some(detail)),
            BodyVerdict::Unverifiable { reason } => ("unverifiable", Some(reason)),
            BodyVerdict::Skipped { reason } => ("skipped", Some(reason)),
        };
        rows.push(TvEvidenceRow {
            phase: if result.label.contains(".loop") {
                "loop".to_string()
            } else {
                "body".to_string()
            },
            label: result.label,
            verdict: verdict.to_string(),
            detail,
        });
    }
}

fn reject_translation_validation(
    evidence: &TranslationValidationEvidence,
    program: &Program,
    closure: &VerifiedClosure,
    exports: &[PlannedExport],
) -> Option<String> {
    for row in &evidence.rows {
        if row.verdict != "faithful" {
            return Some(format!(
                "{} TV `{}` is {}: {}",
                row.phase,
                row.label,
                row.verdict,
                row.detail.as_deref().unwrap_or("no diagnostic")
            ));
        }
    }
    let expected = expected_tv_inventory(program, closure, exports);
    let mut observed: BTreeMap<(String, String), usize> = BTreeMap::new();
    for row in &evidence.rows {
        *observed
            .entry((row.phase.clone(), row.label.clone()))
            .or_default() += 1;
    }
    if observed != expected {
        let missing: Vec<String> = expected
            .iter()
            .filter(|(key, count)| observed.get(*key).copied().unwrap_or(0) != **count)
            .map(|((phase, label), count)| format!("{phase}:{label} x{count}"))
            .collect();
        let unexpected: Vec<String> = observed
            .iter()
            .filter(|(key, count)| expected.get(*key).copied().unwrap_or(0) != **count)
            .map(|((phase, label), count)| format!("{phase}:{label} x{count}"))
            .collect();
        return Some(format!(
            "TV expected/observed inventory mismatch; expected differences [{}], observed differences [{}]",
            missing.join(", "),
            unexpected.join(", ")
        ));
    }
    None
}

fn expected_tv_inventory(
    program: &Program,
    closure: &VerifiedClosure,
    exports: &[PlannedExport],
) -> BTreeMap<(String, String), usize> {
    let mut expected = BTreeMap::new();
    for item in &program.items {
        let Item::Fn(function) = item else {
            continue;
        };
        if !closure.functions.contains(&function.name) {
            continue;
        }
        expect_tv(&mut expected, "contract", format!("{}.req", function.name));
        for index in 0..function.contract.ens.len() {
            expect_tv(
                &mut expected,
                "contract",
                format!("{}.ens#{}", function.name, index + 1),
            );
        }
        if let Some(body) = &function.body {
            let mut loop_index = 0;
            expected_contract_loops(&mut expected, &function.name, body, &mut loop_index);
            let mut let_index = 0;
            for stmt in &body.stmts {
                match stmt {
                    Stmt::Let { .. } => {
                        let_index += 1;
                        expect_tv(
                            &mut expected,
                            "exec",
                            format!("{}.let#{let_index}", function.name),
                        );
                    }
                    Stmt::Return(Some(_)) => {
                        expect_tv(&mut expected, "exec", format!("{}.return", function.name))
                    }
                    _ => {}
                }
            }
            if body.tail.is_some() {
                expect_tv(&mut expected, "exec", format!("{}.tail", function.name));
            }
            let body_label = if matches!(body.stmts.last(), Some(Stmt::Loop(_))) {
                format!("{}.loop", function.name)
            } else {
                function.name.clone()
            };
            let phase = if body_label.contains(".loop") {
                "loop"
            } else {
                "body"
            };
            expect_tv(&mut expected, phase, body_label);
        }
    }
    for export in exports.iter().filter(|export| export.wrapped) {
        expect_tv(
            &mut expected,
            "wrapper_guard",
            format!("{}.export_guard", export.thermite_name),
        );
    }
    expected
}

fn expect_tv(expected: &mut BTreeMap<(String, String), usize>, phase: &str, label: String) {
    *expected.entry((phase.to_string(), label)).or_default() += 1;
}

fn inject_tv_fault(evidence: &mut TranslationValidationEvidence) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Ok(fault) = std::env::var("THERMITE_L3_TEST_FAULT") else {
        return;
    };
    let Some(rest) = fault.strip_prefix("tv-") else {
        return;
    };
    let Some((phase, verdict)) = rest.rsplit_once('-') else {
        return;
    };
    if let Some(row) = evidence.rows.iter_mut().find(|row| row.phase == phase) {
        row.verdict = verdict.to_string();
        row.detail = Some(format!(
            "controlled {verdict} result injected into the {phase} TV phase"
        ));
    }
}

fn expected_contract_loops(
    expected: &mut BTreeMap<(String, String), usize>,
    function: &str,
    block: &Block,
    loop_index: &mut usize,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(node) => {
                *loop_index += 1;
                let current = *loop_index;
                for index in 0..node.invs.len() {
                    expect_tv(
                        expected,
                        "contract",
                        format!("{function}.loop#{current}.inv#{}", index + 1),
                    );
                }
                expect_tv(
                    expected,
                    "contract",
                    format!("{function}.loop#{current}.dec"),
                );
                expected_contract_loops(expected, function, &node.body, loop_index);
            }
            Stmt::If { then, else_, .. } => {
                expected_contract_loops(expected, function, then, loop_index);
                if let Some(else_) = else_ {
                    expected_contract_loops(expected, function, else_, loop_index);
                }
            }
            _ => {}
        }
    }
}

fn collect_toolchain(target: VerifiedTarget) -> Result<CollectedToolchain, ForgeError> {
    let verus = resolve_executable(std::env::var_os("VERUS_BIN").as_deref(), "verus")?;
    let rustup = resolve_executable(None, "rustup")?;
    let verus_version = command_text(Command::new(&verus).arg("--version"), "verus --version")?;
    let artifact_codegen = collect_codegen_rustc(&verus_version, &rustup, target)?;
    let host_rustc = collect_host_rustc(&rustup)?;
    let current = std::env::current_exe().map_err(|source| ForgeError::Io {
        path: "current forge executable".to_string(),
        source,
    })?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let cargo_lock = root.join("Cargo.lock");
    let source_identity = command_text(
        Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(root),
        "git rev-parse HEAD",
    )
    .unwrap_or_else(|_| "unavailable".to_string());
    let environment = closed_verus_environment(&rustup, &artifact_codegen.rustup_toolchain)?;
    let mut link_dependencies = Vec::new();
    let mut dependency_paths = BTreeMap::new();
    let mut kernel_vstd_model = None;
    let mut kernel_vstd_scratch = None;
    let verus_dir = verus.parent().ok_or_else(|| ForgeError::VerusOutput {
        detail: "the resolved Verus binary has no installation directory".to_string(),
    })?;
    if matches!(target, VerifiedTarget::Kernel | VerifiedTarget::KernelUefi) {
        let (scratch, dependencies, model) = build_kernel_vstd_link(KernelVstdLinkRequest {
            verus: &verus,
            verus_dir,
            environment: &environment,
            target,
            codegen: &artifact_codegen,
            source_root_override: None,
            vir_override: None,
            builtin_source_override: None,
        })?;
        for dependency in dependencies {
            dependency_paths.insert(dependency.name.clone(), scratch.path.join(&dependency.name));
            link_dependencies.push(dependency);
        }
        dependency_paths.insert(
            KERNEL_VSTD_SOURCE_PATH_KEY.to_string(),
            verus_dir.join("vstd"),
        );
        dependency_paths.insert(
            KERNEL_VSTD_VIR_PATH_KEY.to_string(),
            verus_dir.join("vstd.vir"),
        );
        if target == VerifiedTarget::KernelUefi {
            dependency_paths.insert(
                KERNEL_BUILTIN_SOURCE_PATH_KEY.to_string(),
                verus_dir.join("builtin/src/lib.rs"),
            );
        }
        kernel_vstd_model = Some(model);
        kernel_vstd_scratch = Some(scratch);
    } else {
        let path = verus_dir.join("libvstd.rlib");
        if !path.is_file() {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "the pinned Verus installation is missing link dependency `{}`",
                    path.display()
                ),
            });
        }
        link_dependencies.push(ToolchainDependency {
            name: "libvstd.rlib".to_string(),
            source_path: path.display().to_string(),
            sha256: file_sha256(&path)?.2,
        });
        dependency_paths.insert("libvstd.rlib".to_string(), path);
    }
    for name in [
        "libverus_builtin.rlib",
        "libverus_builtin_macros.so",
        "libverus_state_machines_macros.so",
    ] {
        if target == VerifiedTarget::KernelUefi && name == "libverus_builtin.rlib" {
            continue;
        }
        let path = verus_dir.join(name);
        if !path.is_file() {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "the pinned Verus installation is missing link dependency `{}`",
                    path.display()
                ),
            });
        }
        link_dependencies.push(ToolchainDependency {
            name: name.to_string(),
            source_path: path.display().to_string(),
            sha256: file_sha256(&path)?.2,
        });
        dependency_paths.insert(name.to_string(), path);
    }
    let z3 = verus_dir.join("z3");
    if !z3.is_file() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "the pinned Verus installation is missing `{}`",
                z3.display()
            ),
        });
    }
    let evidence = ToolchainEvidence {
        forge_version: env!("CARGO_PKG_VERSION").to_string(),
        forge_executable_sha256: file_sha256(&current)?.2,
        forge_source_identity: source_identity,
        verus_path: verus.display().to_string(),
        verus_sha256: file_sha256(&verus)?.2,
        verus_version,
        rustup_path: rustup.display().to_string(),
        rustup_sha256: file_sha256(&rustup)?.2,
        rustup_version: command_text(Command::new(&rustup).arg("--version"), "rustup --version")?,
        host_rustc,
        target_triple: artifact_codegen.target_triple.clone(),
        target_pointer_width: artifact_codegen.target_pointer_width.clone(),
        target_endian: artifact_codegen.target_endian.clone(),
        artifact_codegen,
        z3_path: z3.display().to_string(),
        z3_sha256: file_sha256(&z3)?.2,
        z3_version: command_text(Command::new(&z3).arg("--version"), "pinned z3 --version")?,
        cargo_lock_path: cargo_lock.display().to_string(),
        cargo_lock_sha256: file_sha256(&cargo_lock)?.2,
        link_dependencies,
        kernel_vstd_model,
        source_date_epoch: SOURCE_DATE_EPOCH.to_string(),
        environment,
    };
    Ok(CollectedToolchain {
        evidence,
        dependency_paths,
        _kernel_vstd_scratch: kernel_vstd_scratch,
    })
}

fn kernel_vstd_link_build_args(target: VerifiedTarget) -> Vec<String> {
    let mut args = vec![
        "<VERUS>/vstd/vstd.rs".to_string(),
        "--is-vstd".to_string(),
        "--no-verify".to_string(),
        "--compile".to_string(),
        "--crate-type=rlib".to_string(),
        "--crate-name".to_string(),
        "vstd".to_string(),
        "--out-dir".to_string(),
        "<SCRATCH>".to_string(),
        "--remap-path-prefix=<VSTD>=<VSTD>".to_string(),
        "--remap-path-prefix=<SCRATCH>=<SCRATCH>".to_string(),
    ];
    if target == VerifiedTarget::KernelUefi {
        args.extend([
            "--target".to_string(),
            "x86_64-unknown-uefi".to_string(),
            "--extern".to_string(),
            "verus_builtin=<KERNEL_UEFI_VERUS_BUILTIN_RLIB>".to_string(),
        ]);
    }
    args
}

fn kernel_uefi_builtin_build_args() -> Vec<String> {
    [
        "--crate-name",
        "verus_builtin",
        "--crate-type",
        "rlib",
        "--edition",
        "2018",
        "--target",
        "x86_64-unknown-uefi",
        "--cfg",
        "verus_keep_ghost",
        "--remap-path-prefix=<SCRATCH>=.",
        "--remap-path-prefix=<BUILTIN_SOURCE_ROOT>=<VERUS>/builtin/src",
        "<VERUS>/builtin/src/lib.rs",
        "--out-dir",
        "<SCRATCH>",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

struct KernelVstdLinkRequest<'a> {
    verus: &'a Path,
    verus_dir: &'a Path,
    environment: &'a BTreeMap<String, String>,
    target: VerifiedTarget,
    codegen: &'a CodegenRustcEvidence,
    source_root_override: Option<&'a Path>,
    vir_override: Option<&'a Path>,
    builtin_source_override: Option<&'a Path>,
}

fn build_kernel_vstd_link(
    request: KernelVstdLinkRequest<'_>,
) -> Result<
    (
        ScratchTree,
        Vec<ToolchainDependency>,
        KernelVstdModelEvidence,
    ),
    ForgeError,
> {
    let KernelVstdLinkRequest {
        verus,
        verus_dir,
        environment,
        target,
        codegen,
        source_root_override,
        vir_override,
        builtin_source_override,
    } = request;
    let installed_vir = verus_dir.join("vstd.vir");
    let installed_source_root = verus_dir.join("vstd");
    let vir = vir_override.unwrap_or(&installed_vir);
    let source_root = source_root_override.unwrap_or(&installed_source_root);
    if !vir.is_file() || !source_root.is_dir() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "kernel slice model requires `{}` and `{}`",
                vir.display(),
                source_root.display()
            ),
        });
    }
    let (source_file_count, source_total_bytes, source_sha256) =
        directory_sha256_named(source_root, "thermite.kernel-vstd-source-tree.v1")?;
    let captured_link_source = source_root.join(KERNEL_VSTD_LINK_SOURCE_NAME);
    if !captured_link_source.is_file() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "the pinned Verus vstd tree has no crate root `{}`",
                captured_link_source.display()
            ),
        });
    }
    let scratch = ScratchTree::new_in_temp("kernel_vstd_link")?;
    let compiled_source_root = scratch.path.join("canonical-vstd-source");
    copy_canonical_source_tree(source_root, &compiled_source_root)?;
    write_bytes(
        &compiled_source_root.join(KERNEL_VSTD_LINK_SOURCE_NAME),
        KERNEL_VSTD_LINK_ROOT_SOURCE.as_bytes(),
    )?;
    write_bytes(
        &compiled_source_root.join(KERNEL_VSTD_LINK_PRELUDE_NAME),
        KERNEL_VSTD_LINK_PRELUDE_SOURCE.as_bytes(),
    )?;

    let mut dependencies = Vec::new();
    let mut target_builtin_source_path = None;
    let mut target_builtin_source_sha256 = None;
    let uefi_builtin = if target == VerifiedTarget::KernelUefi {
        let installed_source = verus_dir.join("builtin/src/lib.rs");
        let captured_source = builtin_source_override.unwrap_or(&installed_source);
        if !captured_source.is_file() {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "the pinned Verus installation is missing target builtin source `{}`",
                    captured_source.display()
                ),
            });
        }
        target_builtin_source_path = Some("<VERUS>/builtin/src/lib.rs".to_string());
        target_builtin_source_sha256 = Some(file_sha256(captured_source)?.2);
        let source_root = scratch.path.join("canonical-builtin-source");
        fs::create_dir_all(&source_root).map_err(|source| ForgeError::Io {
            path: source_root.display().to_string(),
            source,
        })?;
        let source = source_root.join("lib.rs");
        write_bytes(
            &source,
            &fs::read(captured_source).map_err(|source_error| ForgeError::Io {
                path: captured_source.display().to_string(),
                source: source_error,
            })?,
        )?;
        let mut command = Command::new(&codegen.rustc_path);
        let source_parent = source.parent().ok_or_else(|| ForgeError::VerusOutput {
            detail: "target verus_builtin source has no parent directory".to_string(),
        })?;
        command
            .args([
                "--crate-name",
                "verus_builtin",
                "--crate-type",
                "rlib",
                "--edition",
                "2018",
                "--target",
                "x86_64-unknown-uefi",
                "--cfg",
                "verus_keep_ghost",
            ])
            .arg(format!("--remap-path-prefix={}=.", scratch.path.display()))
            .arg(format!(
                "--remap-path-prefix={}=<VERUS>/builtin/src",
                source_parent.display()
            ))
            .arg(source)
            .args(["--out-dir", "."])
            .current_dir(&scratch.path)
            .env_clear()
            .envs(environment)
            .env("RUSTC_BOOTSTRAP", "1");
        let output = command
            .output()
            .map_err(|source| ForgeError::VerusSpawn { source })?;
        if !output.status.success() {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "building target-specific verus_builtin failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            });
        }
        let path = scratch.path.join("libverus_builtin.rlib");
        if !path.is_file() {
            return Err(ForgeError::VerusOutput {
                detail: "target-specific verus_builtin produced no rlib".to_string(),
            });
        }
        dependencies.push(ToolchainDependency {
            name: "libverus_builtin.rlib".to_string(),
            source_path: "<forge-generated:uefi-verus-builtin>".to_string(),
            sha256: file_sha256(&path)?.2,
        });
        Some(path)
    } else {
        None
    };

    let mut command = Command::new(verus);
    command
        .arg(Path::new("canonical-vstd-source").join(KERNEL_VSTD_LINK_SOURCE_NAME))
        .args([
            "--is-vstd",
            "--no-verify",
            "--compile",
            "--crate-type=rlib",
            "--crate-name",
            "vstd",
            "--out-dir",
        ])
        .arg(".")
        .arg(format!(
            "--remap-path-prefix={}=<VSTD>",
            compiled_source_root.display()
        ))
        .arg(format!(
            "--remap-path-prefix={}=<SCRATCH>",
            scratch.path.display()
        ));
    if let Some(builtin) = &uefi_builtin {
        if builtin.parent() != Some(scratch.path.as_path()) {
            return Err(ForgeError::VerusOutput {
                detail: "target verus_builtin escaped the canonical vstd scratch root".to_string(),
            });
        }
        command
            .args(["--target", "x86_64-unknown-uefi", "--extern"])
            .arg("verus_builtin=libverus_builtin.rlib");
    }
    command
        .current_dir(&scratch.path)
        .env_clear()
        .envs(environment);
    let output = command
        .output()
        .map_err(|source| ForgeError::VerusSpawn { source })?;
    if !output.status.success() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "building the no_std vstd link crate failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    let rlib = scratch.path.join("libvstd.rlib");
    if !rlib.is_file() {
        return Err(ForgeError::VerusOutput {
            detail: "building the no_std vstd link crate produced no libvstd.rlib".to_string(),
        });
    }
    let link_rlib_sha256 = file_sha256(&rlib)?.2;
    dependencies.push(ToolchainDependency {
        name: "libvstd.rlib".to_string(),
        source_path: "<VERUS>/vstd/vstd.rs".to_string(),
        sha256: link_rlib_sha256.clone(),
    });
    let model = KernelVstdModelEvidence {
        target_triple: codegen.target_triple.clone(),
        vir_path: "<VERUS>/vstd.vir".to_string(),
        vir_sha256: file_sha256(vir)?.2,
        source_root: "<VERUS>/vstd".to_string(),
        source_file_count,
        source_total_bytes,
        source_sha256,
        link_source_name: KERNEL_VSTD_LINK_SOURCE_NAME.to_string(),
        link_source_sha256: sha256(KERNEL_VSTD_LINK_ROOT_SOURCE.as_bytes()),
        link_prelude_sha256: sha256(KERNEL_VSTD_LINK_PRELUDE_SOURCE.as_bytes()),
        link_build_args: kernel_vstd_link_build_args(target),
        link_rlib_sha256,
        target_builtin_source_path,
        target_builtin_source_sha256,
        target_builtin_build_args: if target == VerifiedTarget::KernelUefi {
            kernel_uefi_builtin_build_args()
        } else {
            Vec::new()
        },
    };
    Ok((scratch, dependencies, model))
}

fn closed_verus_environment(
    rustup: &Path,
    rustup_toolchain: &str,
) -> Result<BTreeMap<String, String>, ForgeError> {
    let home = std::env::var("HOME").map_err(|_| ForgeError::VerusOutput {
        detail: "HOME is required to pin the rustup-backed Verus launcher".to_string(),
    })?;
    let rustup_home = std::env::var("RUSTUP_HOME")
        .unwrap_or_else(|_| Path::new(&home).join(".rustup").display().to_string());
    let rustup_dir = rustup.parent().ok_or_else(|| ForgeError::VerusOutput {
        detail: "the resolved rustup executable has no parent directory".to_string(),
    })?;
    let mut path_entries = vec![rustup_dir.to_path_buf()];
    for system in [PathBuf::from("/usr/bin"), PathBuf::from("/bin")] {
        if !path_entries.contains(&system) {
            path_entries.push(system);
        }
    }
    let pinned_path = std::env::join_paths(&path_entries)
        .map_err(|error| ForgeError::VerusOutput {
            detail: format!("could not construct the pinned Verus PATH: {error}"),
        })?
        .to_string_lossy()
        .into_owned();
    Ok(BTreeMap::from([
        ("HOME".to_string(), home),
        ("PATH".to_string(), pinned_path),
        ("RUSTUP_HOME".to_string(), rustup_home),
        ("RUSTUP_TOOLCHAIN".to_string(), rustup_toolchain.to_string()),
        (
            "SOURCE_DATE_EPOCH".to_string(),
            SOURCE_DATE_EPOCH.to_string(),
        ),
    ]))
}

fn collect_host_rustc(rustup: &Path) -> Result<HostRustcEvidence, ForgeError> {
    let selected = command_text(
        Command::new(rustup).args(["which", "rustc"]),
        "rustup which rustc",
    )?;
    let rustc_path = fs::canonicalize(selected.trim()).map_err(|source| ForgeError::Io {
        path: selected.clone(),
        source,
    })?;
    let (_, rustc_sha256) = streamed_file_sha256(&rustc_path)?;
    Ok(HostRustcEvidence {
        rustc_version: command_text(Command::new(&rustc_path).arg("-vV"), "host rustc -vV")?,
        rustc_path: rustc_path.display().to_string(),
        rustc_sha256,
    })
}

fn collect_codegen_rustc(
    verus_version: &str,
    rustup: &Path,
    target: VerifiedTarget,
) -> Result<CodegenRustcEvidence, ForgeError> {
    let rustup_toolchain = parse_verus_toolchain(verus_version)?;
    let selected = command_text(
        Command::new(rustup).args(["which", "--toolchain", rustup_toolchain.as_str(), "rustc"]),
        "rustup which Verus codegen rustc",
    )?;
    let rustc_path = fs::canonicalize(selected.trim()).map_err(|source| ForgeError::Io {
        path: selected.clone(),
        source,
    })?;
    let rustc_version = rustup_command_text(
        rustup,
        &rustup_toolchain,
        &["rustc", "-vV"],
        "Verus codegen rustc -vV",
    )?;
    let host_triple = rustc_version_field(&rustc_version, "host: ")?;
    let target_triple = if target == VerifiedTarget::KernelUefi {
        "x86_64-unknown-uefi".to_string()
    } else {
        host_triple.clone()
    };
    let rustc_release = rustc_version_field(&rustc_version, "release: ")?;
    let rustc_commit_hash = rustc_version_field(&rustc_version, "commit-hash: ")?;
    let llvm_version = rustc_version_field(&rustc_version, "LLVM version: ")?;
    let cfg_args = if target == VerifiedTarget::KernelUefi {
        vec![
            "rustc",
            "--target",
            target_triple.as_str(),
            "--print",
            "cfg",
        ]
    } else {
        vec!["rustc", "--print", "cfg"]
    };
    let cfg = rustup_command_text(
        rustup,
        &rustup_toolchain,
        &cfg_args,
        "Verus codegen rustc --print cfg",
    )?;
    let target_pointer_width = rustc_cfg_value(&cfg, "target_pointer_width")?;
    let target_endian = rustc_cfg_value(&cfg, "target_endian")?;
    let sysroot_text = rustup_command_text(
        rustup,
        &rustup_toolchain,
        &["rustc", "--print", "sysroot"],
        "Verus codegen rustc --print sysroot",
    )?;
    let sysroot = fs::canonicalize(sysroot_text.trim()).map_err(|source| ForgeError::Io {
        path: sysroot_text.clone(),
        source,
    })?;
    if !rustc_path.starts_with(&sysroot) {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "Verus codegen rustc `{}` escapes its rustup sysroot `{}`",
                rustc_path.display(),
                sysroot.display()
            ),
        });
    }
    let target_libdir_args = if target == VerifiedTarget::KernelUefi {
        vec![
            "rustc",
            "--target",
            target_triple.as_str(),
            "--print",
            "target-libdir",
        ]
    } else {
        vec!["rustc", "--print", "target-libdir"]
    };
    let target_libdir_text = rustup_command_text(
        rustup,
        &rustup_toolchain,
        &target_libdir_args,
        "Verus codegen rustc --print target-libdir",
    )?;
    let target_libdir =
        fs::canonicalize(target_libdir_text.trim()).map_err(|source| ForgeError::Io {
            path: target_libdir_text.clone(),
            source,
        })?;
    if !target_libdir.starts_with(&sysroot) {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "Verus codegen target libdir `{}` escapes sysroot `{}`",
                target_libdir.display(),
                sysroot.display()
            ),
        });
    }

    let rustlib = sysroot.join("lib/rustlib");
    let rustc_manifest = rustlib.join(format!("manifest-rustc-{host_triple}"));
    let rust_std_manifest = rustlib.join(format!("manifest-rust-std-{target_triple}"));
    let rustc_driver = component_manifest_largest(
        &rustc_manifest,
        &sysroot,
        |name| name.starts_with("librustc_driver"),
        "rustc driver",
    )?;
    let llvm_library = component_manifest_largest(
        &rustc_manifest,
        &sysroot,
        |name| name.starts_with("libLLVM"),
        "LLVM library",
    )?;
    let (_, rustc_sha256) = streamed_file_sha256(&rustc_path)?;
    let (_, rustc_component_manifest_sha256) = streamed_file_sha256(&rustc_manifest)?;
    let (_, rust_std_component_manifest_sha256) = streamed_file_sha256(&rust_std_manifest)?;
    let (_, rustc_driver_sha256) = streamed_file_sha256(&rustc_driver)?;
    let (_, llvm_library_sha256) = streamed_file_sha256(&llvm_library)?;
    let (target_libdir_file_count, target_libdir_total_bytes, target_libdir_sha256) =
        directory_sha256(&target_libdir)?;

    Ok(CodegenRustcEvidence {
        selection: "verus --version Toolchain".to_string(),
        rustup_toolchain,
        rustc_path: rustc_path.display().to_string(),
        rustc_sha256,
        rustc_version,
        rustc_release,
        rustc_commit_hash,
        sysroot: sysroot.display().to_string(),
        rustc_component_manifest_path: rustc_manifest.display().to_string(),
        rustc_component_manifest_sha256,
        rust_std_component_manifest_path: rust_std_manifest.display().to_string(),
        rust_std_component_manifest_sha256,
        rustc_driver_path: rustc_driver.display().to_string(),
        rustc_driver_sha256,
        llvm_version,
        llvm_library_path: llvm_library.display().to_string(),
        llvm_library_sha256,
        target_triple,
        target_pointer_width,
        target_endian,
        target_libdir: target_libdir.display().to_string(),
        target_libdir_sha256,
        target_libdir_file_count,
        target_libdir_total_bytes,
        linker_identity: "rlib: no final linker invoked by artifact codegen".to_string(),
    })
}

fn parse_verus_toolchain(verus_version: &str) -> Result<String, ForgeError> {
    let mut reported = verus_version
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Toolchain:").map(str::trim));
    let toolchain = reported.next().ok_or_else(|| ForgeError::VerusOutput {
        detail: "pinned Verus did not report an authoritative `Toolchain:` identity".to_string(),
    })?;
    if reported.next().is_some()
        || toolchain.is_empty()
        || !toolchain
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(ForgeError::VerusOutput {
            detail: "pinned Verus did not report a safe authoritative `Toolchain:` identity"
                .to_string(),
        });
    }
    Ok(toolchain.to_string())
}

fn rustup_command_text(
    rustup: &Path,
    toolchain: &str,
    args: &[&str],
    label: &str,
) -> Result<String, ForgeError> {
    command_text(
        Command::new(rustup).arg("run").arg(toolchain).args(args),
        label,
    )
}

fn rustc_version_field(version: &str, prefix: &str) -> Result<String, ForgeError> {
    version
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ForgeError::VerusOutput {
            detail: format!("codegen rustc -vV omitted `{prefix}`"),
        })
}

fn rustc_cfg_value(cfg: &str, key: &str) -> Result<String, ForgeError> {
    let prefix = format!("{key}=\"");
    cfg.lines()
        .find_map(|line| line.strip_prefix(&prefix)?.strip_suffix('"'))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ForgeError::VerusOutput {
            detail: format!("codegen rustc cfg omitted `{key}`"),
        })
}

fn component_manifest_largest(
    manifest: &Path,
    sysroot: &Path,
    matches_name: impl Fn(&str) -> bool,
    label: &str,
) -> Result<PathBuf, ForgeError> {
    let text = fs::read_to_string(manifest).map_err(|source| ForgeError::Io {
        path: manifest.display().to_string(),
        source,
    })?;
    let mut candidates = Vec::new();
    for relative in text.lines().filter_map(|line| line.strip_prefix("file:")) {
        let Some(name) = Path::new(relative)
            .file_name()
            .and_then(|name| name.to_str())
        else {
            continue;
        };
        if matches_name(name) {
            let path = sysroot.join(relative);
            let metadata = fs::metadata(&path).map_err(|source| ForgeError::Io {
                path: path.display().to_string(),
                source,
            })?;
            if metadata.is_file() {
                candidates.push((metadata.len(), path));
            }
        }
    }
    candidates.sort_by(|left, right| left.1.cmp(&right.1));
    candidates
        .into_iter()
        .max_by_key(|(length, _)| *length)
        .map(|(_, path)| path)
        .ok_or_else(|| ForgeError::VerusOutput {
            detail: format!(
                "Verus codegen rustc component manifest `{}` contains no {label}",
                manifest.display()
            ),
        })
}

fn streamed_file_sha256(path: &Path) -> Result<(u64, String), ForgeError> {
    let mut file = File::open(path).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|source| ForgeError::Io {
            path: path.display().to_string(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        length = length.saturating_add(read as u64);
    }
    Ok((length, format!("{:x}", hasher.finalize())))
}

fn directory_sha256(root: &Path) -> Result<(u64, u64, String), ForgeError> {
    directory_sha256_named(root, "thermite.codegen-target-libdir.v1")
}

fn copy_canonical_source_tree(source: &Path, destination: &Path) -> Result<(), ForgeError> {
    fn copy_directory(source: &Path, destination: &Path) -> Result<(), ForgeError> {
        fs::create_dir_all(destination).map_err(|source_error| ForgeError::Io {
            path: destination.display().to_string(),
            source: source_error,
        })?;
        let mut entries = fs::read_dir(source)
            .map_err(|source_error| ForgeError::Io {
                path: source.display().to_string(),
                source: source_error,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source_error| ForgeError::Io {
                path: source.display().to_string(),
                source: source_error,
            })?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let source_path = entry.path();
            let name = entry.file_name();
            let name_text = name.to_str().ok_or_else(|| ForgeError::VerusOutput {
                detail: format!(
                    "kernel vstd source name is not portable UTF-8: {}",
                    source_path.display()
                ),
            })?;
            if [".git", "__pycache__", "target", "dist"].contains(&name_text) {
                return Err(ForgeError::VerusOutput {
                    detail: format!(
                        "kernel vstd canonical source closure contains forbidden incidental path `{}`",
                        source_path.display()
                    ),
                });
            }
            let kind = entry.file_type().map_err(|source_error| ForgeError::Io {
                path: source_path.display().to_string(),
                source: source_error,
            })?;
            let destination_path = destination.join(&name);
            if kind.is_symlink() {
                return Err(ForgeError::VerusOutput {
                    detail: format!(
                        "kernel vstd canonical source closure contains symlink `{}`",
                        source_path.display()
                    ),
                });
            }
            if kind.is_dir() {
                copy_directory(&source_path, &destination_path)?;
            } else if kind.is_file() {
                let bytes = fs::read(&source_path).map_err(|source_error| ForgeError::Io {
                    path: source_path.display().to_string(),
                    source: source_error,
                })?;
                write_bytes(&destination_path, &bytes)?;
            } else {
                return Err(ForgeError::VerusOutput {
                    detail: format!(
                        "kernel vstd canonical source closure contains non-file `{}`",
                        source_path.display()
                    ),
                });
            }
        }
        Ok(())
    }

    copy_directory(source, destination)
}

fn directory_sha256_named(root: &Path, schema: &str) -> Result<(u64, u64, String), ForgeError> {
    fn collect(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), ForgeError> {
        for entry in fs::read_dir(dir).map_err(|source| ForgeError::Io {
            path: dir.display().to_string(),
            source,
        })? {
            let entry = entry.map_err(|source| ForgeError::Io {
                path: dir.display().to_string(),
                source,
            })?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|source| ForgeError::Io {
                path: path.display().to_string(),
                source,
            })?;
            if kind.is_symlink() {
                return Err(ForgeError::VerusOutput {
                    detail: format!(
                        "Verus codegen target libdir contains unsupported symlink `{}`",
                        path.display()
                    ),
                });
            }
            if kind.is_dir() {
                collect(root, &path, files)?;
            } else if kind.is_file() {
                path.strip_prefix(root)
                    .map_err(|_| ForgeError::VerusOutput {
                        detail: "target-lib path escaped its codegen root".to_string(),
                    })?;
                files.push(path);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort_by(|left, right| {
        left.strip_prefix(root)
            .unwrap_or(left)
            .cmp(right.strip_prefix(root).unwrap_or(right))
    });
    let mut canonical = Canonical::new(schema);
    let mut total_bytes = 0_u64;
    for path in &files {
        let relative = path
            .strip_prefix(root)
            .ok()
            .and_then(Path::to_str)
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: format!(
                    "Verus codegen target lib path `{}` is not portable UTF-8",
                    path.display()
                ),
            })?;
        let (length, digest) = streamed_file_sha256(path)?;
        total_bytes = total_bytes.saturating_add(length);
        canonical.record("file", |c| {
            c.field("path", relative);
            c.field("length", &length.to_string());
            c.field("sha256", &digest);
        });
    }
    Ok((files.len() as u64, total_bytes, canonical.finish()))
}

fn validate_codegen_evidence(toolchain: &ToolchainEvidence) -> Result<(), String> {
    let codegen = &toolchain.artifact_codegen;
    let selected = parse_verus_toolchain(&toolchain.verus_version)
        .map_err(|error| format!("cannot recover Verus Toolchain identity: {error}"))?;
    if codegen.selection != "verus --version Toolchain" || codegen.rustup_toolchain != selected {
        return Err(
            "recorded rustup toolchain is not the authoritative Verus `Toolchain:` selection"
                .to_string(),
        );
    }
    let release = rustc_version_field(&codegen.rustc_version, "release: ")
        .map_err(|error| error.to_string())?;
    let commit = rustc_version_field(&codegen.rustc_version, "commit-hash: ")
        .map_err(|error| error.to_string())?;
    let host =
        rustc_version_field(&codegen.rustc_version, "host: ").map_err(|error| error.to_string())?;
    let llvm = rustc_version_field(&codegen.rustc_version, "LLVM version: ")
        .map_err(|error| error.to_string())?;
    let target_matches_codegen =
        codegen.target_triple == host || codegen.target_triple == "x86_64-unknown-uefi";
    if codegen.rustc_release != release
        || codegen.rustc_commit_hash != commit
        || !target_matches_codegen
        || codegen.llvm_version != llvm
        || toolchain.target_triple != codegen.target_triple
        || toolchain.target_pointer_width != codegen.target_pointer_width
        || toolchain.target_endian != codegen.target_endian
    {
        return Err(
            "recorded rustc/LLVM fields disagree with codegen rustc -vV or target facts"
                .to_string(),
        );
    }
    if !matches!(codegen.target_endian.as_str(), "little" | "big")
        || !matches!(
            codegen.target_pointer_width.as_str(),
            "16" | "32" | "64" | "128"
        )
        || codegen.target_libdir_file_count == 0
        || codegen.target_libdir_total_bytes == 0
        || codegen.linker_identity != "rlib: no final linker invoked by artifact codegen"
    {
        return Err("recorded target-library or rlib-linker policy is invalid".to_string());
    }
    for (label, digest) in [
        ("host rustc", toolchain.host_rustc.rustc_sha256.as_str()),
        ("codegen rustc", codegen.rustc_sha256.as_str()),
        (
            "rustc component manifest",
            codegen.rustc_component_manifest_sha256.as_str(),
        ),
        (
            "rust-std component manifest",
            codegen.rust_std_component_manifest_sha256.as_str(),
        ),
        ("rustc driver", codegen.rustc_driver_sha256.as_str()),
        ("LLVM library", codegen.llvm_library_sha256.as_str()),
        ("target library tree", codegen.target_libdir_sha256.as_str()),
    ] {
        if !is_sha256_digest(digest) {
            return Err(format!("{label} has a malformed SHA-256 identity"));
        }
    }
    if toolchain.host_rustc.rustc_version.is_empty() {
        return Err("ambient host rustc evidence is empty".to_string());
    }
    let sysroot = Path::new(&codegen.sysroot);
    if !sysroot.is_absolute()
        || [
            codegen.rustc_path.as_str(),
            codegen.rustc_component_manifest_path.as_str(),
            codegen.rust_std_component_manifest_path.as_str(),
            codegen.rustc_driver_path.as_str(),
            codegen.llvm_library_path.as_str(),
            codegen.target_libdir.as_str(),
        ]
        .iter()
        .any(|path| !Path::new(path).is_absolute() || !Path::new(path).starts_with(sysroot))
    {
        return Err("codegen compiler dependency path escapes its recorded sysroot".to_string());
    }
    Ok(())
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn resolve_executable(
    explicit: Option<&std::ffi::OsStr>,
    fallback: &str,
) -> Result<PathBuf, ForgeError> {
    if let Some(path) = explicit {
        return fs::canonicalize(path).map_err(|source| ForgeError::Io {
            path: Path::new(path).display().to_string(),
            source,
        });
    }
    let Some(path) = std::env::var_os("PATH") else {
        return Err(ForgeError::VerusAbsent {
            binary: fallback.to_string(),
        });
    };
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(fallback);
        if candidate.is_file() {
            return fs::canonicalize(&candidate).map_err(|source| ForgeError::Io {
                path: candidate.display().to_string(),
                source,
            });
        }
    }
    Err(ForgeError::VerusAbsent {
        binary: fallback.to_string(),
    })
}

fn command_text(command: &mut Command, label: &str) -> Result<String, ForgeError> {
    let output = command
        .output()
        .map_err(|source| ForgeError::VerusSpawn { source })?;
    if !output.status.success() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "{label} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct CompiledVerus {
    artifact: Vec<u8>,
    artifact_name: String,
    evidence: VerusEvidence,
}

#[derive(Clone, Copy)]
struct KernelCompileDependencies<'a> {
    vstd_vir: Option<&'a Path>,
    vstd_rlib: Option<&'a Path>,
    verus_builtin_rlib: Option<&'a Path>,
}

fn compile_verus_source(
    crate_name: &str,
    source: &str,
    target: VerifiedTarget,
    verus_path: &str,
    environment: &BTreeMap<String, String>,
    codegen_toolchain_sha256: &str,
    kernel_dependencies: KernelCompileDependencies<'_>,
) -> Result<CompiledVerus, ForgeError> {
    let scratch = ScratchTree::new_in_temp(&format!("verified_{crate_name}"))?;
    let source_name = format!("{crate_name}.rs");
    let source_path = scratch.path.join(&source_name);
    write_bytes(&source_path, source.as_bytes())?;
    let before = file_sha256(&source_path)?.2;
    let args = expected_verus_args(crate_name, target);
    let mut command = Command::new(verus_path);
    for arg in &args[..args.len() - 2] {
        match arg.as_str() {
            "vstd=<KERNEL_VSTD_VIR>" => {
                let vir = kernel_dependencies
                    .vstd_vir
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "kernel verification has no bound vstd VIR".to_string(),
                    })?;
                command.arg(format!("vstd={}", vir.display()));
            }
            "vstd=<KERNEL_VSTD_RLIB>" => {
                let rlib =
                    kernel_dependencies
                        .vstd_rlib
                        .ok_or_else(|| ForgeError::VerusOutput {
                            detail: "kernel verification has no generated no_std vstd link crate"
                                .to_string(),
                        })?;
                command.arg(format!("vstd={}", rlib.display()));
            }
            "verus_builtin=<KERNEL_UEFI_VERUS_BUILTIN_RLIB>" => {
                let rlib = kernel_dependencies.verus_builtin_rlib.ok_or_else(|| {
                    ForgeError::VerusOutput {
                        detail: "UEFI kernel verification has no target-specific verus_builtin"
                            .to_string(),
                    }
                })?;
                command.arg(format!("verus_builtin={}", rlib.display()));
            }
            _ => {
                command.arg(arg);
            }
        }
    }
    command
        .arg(format!("--remap-path-prefix={}=.", scratch.path.display()))
        .arg(&source_name)
        .current_dir(&scratch.path);
    // The final verifier/codegen process receives a closed environment. This is
    // stronger than attempting to enumerate every ambient variable a future
    // rustc/LLVM or linker release might interpret.
    command.env_clear();
    command.envs(environment);
    let output = command.output().map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            ForgeError::VerusAbsent {
                binary: verus_path.to_string(),
            }
        } else {
            ForgeError::VerusSpawn { source }
        }
    })?;
    let after = file_sha256(&source_path)?.2;
    let stdout_raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stdout = normalize_json_output(&stdout_raw).unwrap_or(stdout_raw);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let (reported_success, errors) = parse_verus_summary(&stdout);
    let success = output.status.success() && reported_success && errors == Some(0);
    if stdout.contains("cheating") || stderr.contains("cheating") {
        return Err(ForgeError::VerusOutput {
            detail: format!("strict Verus invocation reported cheating: {stderr}"),
        });
    }
    let evidence = VerusEvidence {
        args,
        source_relative_path: source_name,
        source_sha256_before: before,
        source_sha256_after: after,
        codegen_toolchain_sha256: codegen_toolchain_sha256.to_string(),
        success,
        errors,
        stdout,
        stderr,
        scopes: Vec::new(),
    };
    if !success {
        return Ok(CompiledVerus {
            artifact: Vec::new(),
            artifact_name: format!("lib{crate_name}.rlib"),
            evidence,
        });
    }
    let expected = scratch.path.join(format!("lib{crate_name}.rlib"));
    let artifact_path = if expected.is_file() {
        expected
    } else {
        let candidates: Vec<PathBuf> = fs::read_dir(&scratch.path)
            .map_err(|source| ForgeError::Io {
                path: scratch.path.display().to_string(),
                source,
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rlib"))
            .collect();
        if candidates.len() != 1 {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "strict Verus compile succeeded but produced {} rlib candidates",
                    candidates.len()
                ),
            });
        }
        candidates[0].clone()
    };
    let artifact_name = artifact_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ForgeError::VerusOutput {
            detail: "Verus artifact has a non-UTF-8 filename".to_string(),
        })?
        .to_string();
    let artifact = fs::read(&artifact_path).map_err(|source| ForgeError::Io {
        path: artifact_path.display().to_string(),
        source,
    })?;
    Ok(CompiledVerus {
        artifact,
        artifact_name,
        evidence,
    })
}

fn expected_verus_args(crate_name: &str, target: VerifiedTarget) -> Vec<String> {
    let mut args = vec!["--output-json".to_string(), "--profile".to_string()];
    if matches!(target, VerifiedTarget::Kernel | VerifiedTarget::KernelUefi) {
        args.extend([
            "--no-vstd".to_string(),
            "--import".to_string(),
            "vstd=<KERNEL_VSTD_VIR>".to_string(),
            "--extern".to_string(),
            "vstd=<KERNEL_VSTD_RLIB>".to_string(),
        ]);
    }
    if target == VerifiedTarget::KernelUefi {
        args.extend([
            "--extern".to_string(),
            "verus_builtin=<KERNEL_UEFI_VERUS_BUILTIN_RLIB>".to_string(),
            "--target".to_string(),
            "x86_64-unknown-uefi".to_string(),
        ]);
    }
    args.extend([
        "--no-cheating".to_string(),
        "--compile".to_string(),
        "--rlimit".to_string(),
        DEFAULT_RLIMIT.to_string(),
        "--smt-option".to_string(),
        format!("smt.random_seed={DEFAULT_SOLVER_SEED}"),
        "-C".to_string(),
        "panic=abort".to_string(),
        "--remap-path-prefix=<SCRATCH>=.".to_string(),
        format!("{crate_name}.rs"),
    ]);
    args
}

fn expected_verus_scope_args(
    crate_name: &str,
    target: VerifiedTarget,
    module: Option<&str>,
) -> Vec<String> {
    let mut args = vec!["--output-json".to_string(), "--profile".to_string()];
    if matches!(target, VerifiedTarget::Kernel | VerifiedTarget::KernelUefi) {
        args.extend([
            "--no-vstd".to_string(),
            "--import".to_string(),
            "vstd=<KERNEL_VSTD_VIR>".to_string(),
            "--extern".to_string(),
            "vstd=<KERNEL_VSTD_RLIB>".to_string(),
        ]);
    }
    if target == VerifiedTarget::KernelUefi {
        args.extend([
            "--extern".to_string(),
            "verus_builtin=<KERNEL_UEFI_VERUS_BUILTIN_RLIB>".to_string(),
            "--target".to_string(),
            "x86_64-unknown-uefi".to_string(),
        ]);
    }
    args.push("--no-cheating".to_string());
    match module {
        Some(module) => args.extend(["--verify-only-module".to_string(), module.to_string()]),
        None => args.push("--verify-root".to_string()),
    }
    args.extend([
        "--rlimit".to_string(),
        DEFAULT_RLIMIT.to_string(),
        "--smt-option".to_string(),
        format!("smt.random_seed={DEFAULT_SOLVER_SEED}"),
        "--remap-path-prefix=<SCRATCH>=.".to_string(),
        format!("{crate_name}.rs"),
    ]);
    args
}

fn verify_verus_scope(
    crate_name: &str,
    source: &str,
    target: VerifiedTarget,
    scope: &str,
    source_policy: &str,
    module: Option<&str>,
    verus_path: &str,
    environment: &BTreeMap<String, String>,
    codegen_toolchain_sha256: &str,
    kernel_dependencies: KernelCompileDependencies<'_>,
) -> Result<VerusScopeEvidence, ForgeError> {
    let scratch = ScratchTree::new_in_temp(&format!("verified_scope_{crate_name}_{scope}"))?;
    let source_name = format!("{crate_name}.rs");
    let source_path = scratch.path.join(&source_name);
    write_bytes(&source_path, source.as_bytes())?;
    let before = file_sha256(&source_path)?.2;
    let args = expected_verus_scope_args(crate_name, target, module);
    let mut command = Command::new(verus_path);
    for arg in &args[..args.len() - 2] {
        match arg.as_str() {
            "vstd=<KERNEL_VSTD_VIR>" => {
                let vir = kernel_dependencies
                    .vstd_vir
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "kernel scoped verification has no bound vstd VIR".to_string(),
                    })?;
                command.arg(format!("vstd={}", vir.display()));
            }
            "vstd=<KERNEL_VSTD_RLIB>" => {
                let rlib =
                    kernel_dependencies
                        .vstd_rlib
                        .ok_or_else(|| ForgeError::VerusOutput {
                            detail:
                                "kernel scoped verification has no generated no_std vstd link crate"
                                    .to_string(),
                        })?;
                command.arg(format!("vstd={}", rlib.display()));
            }
            "verus_builtin=<KERNEL_UEFI_VERUS_BUILTIN_RLIB>" => {
                let rlib = kernel_dependencies.verus_builtin_rlib.ok_or_else(|| {
                    ForgeError::VerusOutput {
                        detail: "UEFI scoped verification has no target-specific verus_builtin"
                            .to_string(),
                    }
                })?;
                command.arg(format!("verus_builtin={}", rlib.display()));
            }
            _ => {
                command.arg(arg);
            }
        }
    }
    command
        .arg(format!("--remap-path-prefix={}=.", scratch.path.display()))
        .arg(&source_name)
        .current_dir(&scratch.path)
        .env_clear()
        .envs(environment);
    let output = command.output().map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            ForgeError::VerusAbsent {
                binary: verus_path.to_string(),
            }
        } else {
            ForgeError::VerusSpawn { source }
        }
    })?;
    let after = file_sha256(&source_path)?.2;
    let stdout_raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stdout = normalize_json_output(&stdout_raw).unwrap_or(stdout_raw);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let (reported_success, errors) = parse_verus_summary(&stdout);
    let success = output.status.success() && reported_success && errors == Some(0);
    if stdout.contains("cheating") || stderr.contains("cheating") {
        return Err(ForgeError::VerusOutput {
            detail: format!("scoped Verus invocation `{scope}` reported cheating: {stderr}"),
        });
    }
    let evidence = VerusScopeEvidence {
        scope: scope.to_string(),
        source_policy: source_policy.to_string(),
        args,
        source_relative_path: source_name,
        source_sha256_before: before,
        source_sha256_after: after,
        codegen_toolchain_sha256: codegen_toolchain_sha256.to_string(),
        success,
        errors,
        stdout,
        stderr,
    };
    if !evidence.success {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "scoped Verus attribution `{scope}` failed (errors={:?}): {}",
                evidence.errors, evidence.stderr
            ),
        });
    }
    Ok(evidence)
}

fn collect_composition_scope_evidence(
    crate_name: &str,
    source: &str,
    target: VerifiedTarget,
    shell_modules: &[PlannedShellModule],
    verus_path: &str,
    environment: &BTreeMap<String, String>,
    codegen_toolchain_sha256: &str,
    kernel_dependencies: KernelCompileDependencies<'_>,
) -> Result<Vec<VerusScopeEvidence>, ForgeError> {
    let mut scopes = vec![verify_verus_scope(
        crate_name,
        source,
        target,
        "lowered-thermite",
        "generated_thermite_v1",
        None,
        verus_path,
        environment,
        codegen_toolchain_sha256,
        kernel_dependencies,
    )?];
    for module in shell_modules {
        scopes.push(verify_verus_scope(
            crate_name,
            source,
            target,
            &module.name,
            &module.source_policy,
            Some(&module.name),
            verus_path,
            environment,
            codegen_toolchain_sha256,
            kernel_dependencies,
        )?);
    }
    Ok(scopes)
}

fn parse_verus_summary(stdout: &str) -> (bool, Option<u64>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
        return (false, None);
    };
    if let Some(summary) = value.get("verification-results") {
        return (
            summary
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            summary.get("errors").and_then(|v| v.as_u64()),
        );
    }
    (false, None)
}

fn parse_verus_verified_count(stdout: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(stdout.trim())
        .ok()?
        .pointer("/verification-results/verified")?
        .as_u64()
}

fn validate_verus_scope_evidence(
    plan: &ArtifactPlanV1,
    evidence: &VerusEvidence,
) -> Result<(), ForgeError> {
    let Some(composition) = &plan.composition else {
        if evidence.scopes.is_empty() {
            return Ok(());
        }
        return Err(ForgeError::VerusOutput {
            detail: "non-composition Verus evidence contains scoped attribution".to_string(),
        });
    };
    let mut expected = vec![(
        "lowered-thermite".to_string(),
        "generated_thermite_v1".to_string(),
        None,
    )];
    expected.extend(composition.shell_modules.iter().map(|module| {
        (
            module.name.clone(),
            module.source_policy.clone(),
            Some(module.name.clone()),
        )
    }));
    if evidence.scopes.len() != expected.len() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "scoped Verus attribution has {} rows, expected {}",
                evidence.scopes.len(),
                expected.len()
            ),
        });
    }
    let mut attributed = 0_u64;
    for (scope, (expected_scope, expected_policy, module)) in evidence.scopes.iter().zip(expected) {
        let verified =
            parse_verus_verified_count(&scope.stdout).ok_or_else(|| ForgeError::VerusOutput {
                detail: format!(
                    "scoped Verus attribution `{}` omitted its verified-obligation count",
                    scope.scope
                ),
            })?;
        if scope.scope != expected_scope
            || scope.source_policy != expected_policy
            || scope.args
                != expected_verus_scope_args(&plan.crate_name, plan.target, module.as_deref())
            || scope.source_relative_path != format!("{}.rs", plan.crate_name)
            || scope.source_sha256_before != plan.expected_verus_source_sha256
            || scope.source_sha256_after != plan.expected_verus_source_sha256
            || scope.codegen_toolchain_sha256 != evidence.codegen_toolchain_sha256
            || !scope.success
            || scope.errors != Some(0)
            || parse_verus_summary(&scope.stdout) != (true, Some(0))
            || verified == 0
        {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "scoped Verus attribution `{}` is not a successful proof of its exact bound module",
                    scope.scope
                ),
            });
        }
        attributed = attributed
            .checked_add(verified)
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: "scoped Verus obligation total overflowed".to_string(),
            })?;
    }
    let whole_crate =
        parse_verus_verified_count(&evidence.stdout).ok_or_else(|| ForgeError::VerusOutput {
            detail: "whole-crate Verus evidence omitted its verified-obligation count".to_string(),
        })?;
    if attributed != whole_crate {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "scoped Verus obligations total {attributed}, but whole-crate Verus reports {whole_crate}"
            ),
        });
    }
    Ok(())
}

fn verus_failure_detail(label: &str, evidence: &VerusEvidence) -> String {
    match evidence.errors {
        Some(errors) => format!("{label} (errors={errors}): {}", evidence.stderr),
        None => format!("{label}: {}", evidence.stderr),
    }
}

fn normalize_json_output(text: &str) -> Option<String> {
    fn sort(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(object) => {
                let mut entries: Vec<_> = object.into_iter().collect();
                entries.sort_by(|(left, _), (right, _)| left.cmp(right));
                let mut sorted = serde_json::Map::new();
                for (key, value) in entries {
                    sorted.insert(key, sort(value));
                }
                serde_json::Value::Object(sorted)
            }
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(sort).collect())
            }
            other => other,
        }
    }
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let mut rendered = serde_json::to_string_pretty(&sort(value)).ok()?;
    rendered.push('\n');
    Some(rendered)
}

struct StageInput<'a> {
    destination: &'a Path,
    crate_name: &'a str,
    target: VerifiedTarget,
    raw_source: &'a [u8],
    package: Option<&'a crate::thermite_package::LoadedPackage>,
    plan: &'a ArtifactPlanV1,
    plan_sha256: &'a str,
    verus_source: &'a str,
    certificates: &'a [Certificate],
    tv: &'a TranslationValidationEvidence,
    compiled: &'a CompiledVerus,
    toolchain: &'a ToolchainEvidence,
    dependency_paths: &'a BTreeMap<String, PathBuf>,
    composition: Option<CompositionStageInput<'a>>,
}

struct CompositionStageInput<'a> {
    lowered_thermite: &'a str,
    shell_sources: &'a [DirectVerusSource],
}

fn stage_and_publish(input: StageInput<'_>) -> Result<VerifiedBuildReceiptV1, ForgeError> {
    let StageInput {
        destination,
        crate_name,
        target,
        raw_source,
        package,
        plan,
        plan_sha256,
        verus_source,
        certificates,
        tv,
        compiled,
        toolchain,
        dependency_paths,
        composition,
    } = input;
    let parent = destination.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|source| ForgeError::Io {
        path: parent.display().to_string(),
        source,
    })?;
    let stage = ScratchTree::new_sibling(destination)?;
    let evidence = stage.path.join("evidence");
    let artifact_dir = stage.path.join("artifact");
    fs::create_dir_all(&evidence).map_err(|source| ForgeError::Io {
        path: evidence.display().to_string(),
        source,
    })?;
    fs::create_dir_all(&artifact_dir).map_err(|source| ForgeError::Io {
        path: artifact_dir.display().to_string(),
        source,
    })?;
    let dependency_dir = artifact_dir.join("deps");
    fs::create_dir_all(&dependency_dir).map_err(|source| ForgeError::Io {
        path: dependency_dir.display().to_string(),
        source,
    })?;

    let plan_json = pretty_json(plan, "artifact plan")?;
    let mut stable_certificates = certificates.to_vec();
    for certificate in &mut stable_certificates {
        certificate.solver_time_ms = 0;
        certificate.cached = false;
        certificate.solver_profile = None;
    }
    let cert_json = pretty_json(&stable_certificates, "certificate set")?;
    let tv_json = pretty_json(tv, "translation-validation evidence")?;
    let verus_json = pretty_json(&compiled.evidence, "whole-crate Verus evidence")?;
    let toolchain_json = pretty_json(toolchain, "toolchain evidence")?;
    write_bytes(&evidence.join("input.th"), raw_source)?;
    if let Some(package) = package {
        crate::thermite_package::write_evidence(&stage.path, package)?;
    }
    write_bytes(&evidence.join("artifact-plan.v1"), plan_json.as_bytes())?;
    write_bytes(&evidence.join("source.verus.rs"), verus_source.as_bytes())?;
    if let Some(composition) = &composition {
        let shell_dir = evidence.join("direct-verus");
        fs::create_dir_all(&shell_dir).map_err(|source| ForgeError::Io {
            path: shell_dir.display().to_string(),
            source,
        })?;
        write_bytes(
            &evidence.join("lowered-thermite.verus.rs"),
            composition.lowered_thermite.as_bytes(),
        )?;
        for shell in composition.shell_sources {
            write_bytes(&stage.path.join(&shell.plan.path), &shell.bytes)?;
        }
    }
    write_bytes(&evidence.join("certificates.json"), cert_json.as_bytes())?;
    write_bytes(
        &evidence.join("translation-validation.json"),
        tv_json.as_bytes(),
    )?;
    write_bytes(&evidence.join("verus-result.json"), verus_json.as_bytes())?;
    write_bytes(&evidence.join("toolchain.json"), toolchain_json.as_bytes())?;
    if let Some(model) = &toolchain.kernel_vstd_model {
        let source_root = dependency_paths
            .get(KERNEL_VSTD_SOURCE_PATH_KEY)
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: "kernel build did not retain its vstd source root".to_string(),
            })?;
        let staged_source_root = evidence.join(KERNEL_VSTD_SOURCE_EVIDENCE_DIR);
        copy_canonical_source_tree(source_root, &staged_source_root)?;
        if directory_sha256_named(&staged_source_root, "thermite.kernel-vstd-source-tree.v1")?
            != (
                model.source_file_count,
                model.source_total_bytes,
                model.source_sha256.clone(),
            )
        {
            return Err(ForgeError::VerusOutput {
                detail: "kernel vstd source closure changed after toolchain capture".to_string(),
            });
        }
        let vir_path = dependency_paths
            .get(KERNEL_VSTD_VIR_PATH_KEY)
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: "kernel build did not retain its vstd VIR input".to_string(),
            })?;
        let vir = fs::read(vir_path).map_err(|source| ForgeError::Io {
            path: vir_path.display().to_string(),
            source,
        })?;
        if sha256(&vir) != model.vir_sha256 {
            return Err(ForgeError::VerusOutput {
                detail: "kernel vstd VIR changed after toolchain capture".to_string(),
            });
        }
        write_bytes(&evidence.join(KERNEL_VSTD_VIR_EVIDENCE_NAME), &vir)?;
        let link_evidence = evidence.join(KERNEL_VSTD_LINK_EVIDENCE_DIR);
        fs::create_dir_all(&link_evidence).map_err(|source| ForgeError::Io {
            path: link_evidence.display().to_string(),
            source,
        })?;
        if sha256(KERNEL_VSTD_LINK_ROOT_SOURCE.as_bytes()) != model.link_source_sha256
            || sha256(KERNEL_VSTD_LINK_PRELUDE_SOURCE.as_bytes()) != model.link_prelude_sha256
        {
            return Err(ForgeError::VerusOutput {
                detail: "kernel vstd selected link sources changed after capture".to_string(),
            });
        }
        write_bytes(
            &link_evidence.join(KERNEL_VSTD_LINK_SOURCE_NAME),
            KERNEL_VSTD_LINK_ROOT_SOURCE.as_bytes(),
        )?;
        write_bytes(
            &link_evidence.join(KERNEL_VSTD_LINK_PRELUDE_NAME),
            KERNEL_VSTD_LINK_PRELUDE_SOURCE.as_bytes(),
        )?;
        if let Some(expected) = model.target_builtin_source_sha256.as_deref() {
            let path = dependency_paths
                .get(KERNEL_BUILTIN_SOURCE_PATH_KEY)
                .ok_or_else(|| ForgeError::VerusOutput {
                    detail: "UEFI build did not retain its verus_builtin source".to_string(),
                })?;
            let bytes = fs::read(path).map_err(|source| ForgeError::Io {
                path: path.display().to_string(),
                source,
            })?;
            if sha256(&bytes) != expected {
                return Err(ForgeError::VerusOutput {
                    detail: "kernel verus_builtin source changed after capture".to_string(),
                });
            }
            let parent = evidence.join("kernel-verus-builtin");
            fs::create_dir_all(&parent).map_err(|source| ForgeError::Io {
                path: parent.display().to_string(),
                source,
            })?;
            write_bytes(&evidence.join(KERNEL_BUILTIN_EVIDENCE_NAME), &bytes)?;
        }
    }
    let cargo_lock = fs::read(&toolchain.cargo_lock_path).map_err(|source| ForgeError::Io {
        path: toolchain.cargo_lock_path.clone(),
        source,
    })?;
    if sha256(&cargo_lock) != toolchain.cargo_lock_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "Cargo.lock changed after toolchain capture".to_string(),
        });
    }
    write_bytes(&evidence.join("Cargo.lock"), &cargo_lock)?;
    let artifact_relative = format!("artifact/{}", compiled.artifact_name);
    write_bytes(&stage.path.join(&artifact_relative), &compiled.artifact)?;
    for dependency in &toolchain.link_dependencies {
        let source_path =
            dependency_paths
                .get(&dependency.name)
                .ok_or_else(|| ForgeError::VerusOutput {
                    detail: format!(
                        "no captured source for link dependency `{}`",
                        dependency.name
                    ),
                })?;
        let bytes = fs::read(source_path).map_err(|source| ForgeError::Io {
            path: source_path.display().to_string(),
            source,
        })?;
        if sha256(&bytes) != dependency.sha256 {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "link dependency `{}` changed after toolchain capture",
                    dependency.name
                ),
            });
        }
        write_bytes(&dependency_dir.join(&dependency.name), &bytes)?;
    }

    let mut files = collect_bound_files(&stage.path)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let artifact_file = files
        .iter()
        .find(|file| file.path == artifact_relative)
        .cloned()
        .ok_or_else(|| ForgeError::VerusOutput {
            detail: "staged bundle inventory omitted the compiled artifact".to_string(),
        })?;
    let artifact = BoundArtifact {
        path: artifact_file.path,
        kind: "rlib".to_string(),
        length: artifact_file.length,
        sha256: artifact_file.sha256,
    };
    let direct_refined_boundary_names: BTreeSet<String> = plan
        .composition
        .as_ref()
        .map(|composition| {
            composition
                .direct_refined_boundaries
                .iter()
                .map(|boundary| boundary.thermite_name.clone())
                .collect()
        })
        .unwrap_or_default();
    let assurance_aggregate = assurance_aggregate(
        certificates,
        &VerifiedClosure {
            roots: plan_roots(plan),
            functions: plan
                .closure_nodes
                .iter()
                .filter(|node| node.kind == "fn")
                .map(|node| node.name.clone())
                .collect(),
            spec_functions: plan
                .closure_nodes
                .iter()
                .filter(|node| node.kind == "spec_fn")
                .map(|node| node.name.clone())
                .collect(),
            edges: plan
                .closure_edges
                .iter()
                .map(|edge| (edge[0].clone(), edge[1].clone()))
                .collect(),
        },
        &plan.exports,
        &direct_refined_boundary_names,
    )?;
    let injected_mutation = match std::env::var("THERMITE_L3_TEST_FAULT").as_deref() {
        Ok("after-artifact-hash") if cfg!(debug_assertions) => Some(artifact_relative.as_str()),
        Ok("after-plan-hash") if cfg!(debug_assertions) => Some("evidence/artifact-plan.v1"),
        Ok("after-evidence-hash") if cfg!(debug_assertions) => Some("evidence/certificates.json"),
        Ok("after-toolchain-hash") if cfg!(debug_assertions) => Some("evidence/toolchain.json"),
        _ => None,
    };
    if let Some(relative) = injected_mutation {
        let path = stage.path.join(relative);
        let mut bytes = fs::read(&path).map_err(|source| ForgeError::Io {
            path: path.display().to_string(),
            source,
        })?;
        bytes.push(b' ');
        write_bytes(&path, &bytes)?;
    }
    let receipt_schema = if plan.composition.is_some() {
        COMPOSITION_RECEIPT_SCHEMA
    } else {
        RECEIPT_SCHEMA
    };
    let composition_binding =
        plan.composition
            .as_ref()
            .map(|composition| CompositionReceiptBindingV1 {
                lowered_thermite_sha256: composition.lowered_thermite_sha256.clone(),
                direct_verus_set_sha256: canonical_shell_set_sha256(&composition.shell_modules),
                inventory_sha256: canonical_composition_inventory_sha256(&composition.inventory),
                combined_source_sha256: composition.combined_source_sha256.clone(),
            });
    let binding = ReceiptBindingV1 {
        schema: receipt_schema.to_string(),
        assurance: "L3".to_string(),
        scope: "end_to_end".to_string(),
        plan_sha256: plan_sha256.to_string(),
        raw_source_sha256: plan.raw_source_sha256.clone(),
        parsed_program_sha256: plan.parsed_program_sha256.clone(),
        verus_source_sha256: plan.expected_verus_source_sha256.clone(),
        certificate_set_sha256: sha256(cert_json.as_bytes()),
        translation_validation_sha256: sha256(tv_json.as_bytes()),
        whole_crate_verus_sha256: sha256(verus_json.as_bytes()),
        toolchain_sha256: sha256(toolchain_json.as_bytes()),
        crate_name: crate_name.to_string(),
        target,
        artifact,
        assurance_aggregate,
        exports: plan.exports.clone(),
        strict_gates: plan.strict_gates.clone(),
        files,
        composition: composition_binding,
    };
    let receipt = VerifiedBuildReceiptV1 {
        schema: receipt_schema.to_string(),
        binding_sha256: binding.canonical_sha256(),
        binding,
    };
    let receipt_json = pretty_json(&receipt, "verified-build receipt")?;
    write_bytes(&stage.path.join("receipt.json"), receipt_json.as_bytes())?;
    if test_fault("after-receipt-staging") {
        return Err(ForgeError::VerusOutput {
            detail: "injected failure after receipt staging".to_string(),
        });
    }
    sync_tree(&stage.path)?;
    validate_bundle(&stage.path, false)?;
    fs::rename(&stage.path, destination).map_err(|source| ForgeError::Io {
        path: format!("{} -> {}", stage.path.display(), destination.display()),
        source,
    })?;
    stage.disarm();
    sync_dir(parent)?;
    Ok(receipt)
}

fn plan_roots(plan: &ArtifactPlanV1) -> Vec<String> {
    let mut roots: Vec<String> = plan
        .exports
        .iter()
        .map(|export| export.thermite_name.clone())
        .collect();
    if let Some(composition) = &plan.composition {
        roots.extend(
            composition
                .composition_exports
                .iter()
                .map(|export| export.thermite_name.clone()),
        );
    }
    roots.sort();
    roots.dedup();
    roots
}

fn pretty_json<T: Serialize>(value: &T, label: &str) -> Result<String, ForgeError> {
    serde_json::to_string_pretty(value).map_err(|error| ForgeError::VerusOutput {
        detail: format!("could not serialize {label}: {error}"),
    })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), ForgeError> {
    let mut file = File::create(path).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    file.write_all(bytes).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    file.sync_all().map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn file_sha256(path: &Path) -> Result<(u64, Vec<u8>, String), ForgeError> {
    let bytes = fs::read(path).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok((bytes.len() as u64, bytes.clone(), sha256(&bytes)))
}

fn collect_bound_files(root: &Path) -> Result<Vec<BoundFile>, ForgeError> {
    let mut files = Vec::new();
    collect_files_recursive(root, root, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    root: &Path,
    dir: &Path,
    files: &mut Vec<BoundFile>,
) -> Result<(), ForgeError> {
    for entry in fs::read_dir(dir).map_err(|source| ForgeError::Io {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| ForgeError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let file_type = entry.file_type().map_err(|source| ForgeError::Io {
            path: entry.path().display().to_string(),
            source,
        })?;
        if file_type.is_symlink() {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "bundle contains forbidden symlink `{}`",
                    entry.path().display()
                ),
            });
        }
        if file_type.is_dir() {
            collect_files_recursive(root, &entry.path(), files)?;
        } else if file_type.is_file() {
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| ForgeError::VerusOutput {
                    detail: "bundle path escaped its root".to_string(),
                })?
                .to_string_lossy()
                .replace('\\', "/");
            if relative == "receipt.json" {
                continue;
            }
            let (length, _, digest) = file_sha256(&entry.path())?;
            files.push(BoundFile {
                path: relative,
                length,
                sha256: digest,
            });
        }
    }
    Ok(())
}

pub fn validate_bundle(bundle: &Path, replay: bool) -> Result<VerifyBuildReport, ForgeError> {
    let receipt_path = bundle.join("receipt.json");
    let receipt_bytes = fs::read(&receipt_path).map_err(|source| ForgeError::Io {
        path: receipt_path.display().to_string(),
        source,
    })?;
    let receipt: VerifiedBuildReceiptV1 =
        serde_json::from_slice(&receipt_bytes).map_err(|error| ForgeError::VerusOutput {
            detail: format!("invalid verified-build receipt: {error}"),
        })?;
    let composition_receipt = receipt.schema == COMPOSITION_RECEIPT_SCHEMA
        && receipt.binding.schema == COMPOSITION_RECEIPT_SCHEMA;
    let ordinary_receipt =
        receipt.schema == RECEIPT_SCHEMA && receipt.binding.schema == RECEIPT_SCHEMA;
    if !ordinary_receipt && !composition_receipt {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "unsupported verified-build receipt schema `{}`",
                receipt.schema
            ),
        });
    }
    if receipt.binding.canonical_sha256() != receipt.binding_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "verified-build binding digest mismatch".to_string(),
        });
    }
    let mandatory_policy = if composition_receipt {
        COMPOSITION_STRICT_GATES
    } else {
        STRICT_GATES
    };
    let mandatory: Vec<String> = mandatory_policy
        .iter()
        .map(|gate| (*gate).to_string())
        .collect();
    if receipt.binding.strict_gates != mandatory {
        return Err(ForgeError::VerusOutput {
            detail: "verified-build receipt does not carry the exact mandatory strict-gate policy"
                .to_string(),
        });
    }
    let observed = collect_bound_files(bundle)?;
    let expected: BTreeMap<&str, &BoundFile> = receipt
        .binding
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    if expected.len() != receipt.binding.files.len() || observed.len() != expected.len() {
        return Err(ForgeError::VerusOutput {
            detail: "bundle file inventory has missing, duplicate, or extra paths".to_string(),
        });
    }
    for file in &receipt.binding.files {
        validate_relative_path(&file.path)?;
        let path = bundle.join(&file.path);
        let metadata = fs::symlink_metadata(&path).map_err(|source| ForgeError::Io {
            path: path.display().to_string(),
            source,
        })?;
        if !metadata.file_type().is_file() {
            return Err(ForgeError::VerusOutput {
                detail: format!("bound path `{}` is not a regular file", file.path),
            });
        }
        let (length, _, digest) = file_sha256(&path)?;
        if length != file.length || digest != file.sha256 {
            return Err(ForgeError::VerusOutput {
                detail: format!("bound file `{}` failed its length/digest check", file.path),
            });
        }
    }
    let observed_set: BTreeSet<&str> = observed.iter().map(|file| file.path.as_str()).collect();
    if observed_set != expected.keys().copied().collect() {
        return Err(ForgeError::VerusOutput {
            detail: "bundle contains an unbound file or omits a bound file".to_string(),
        });
    }

    let plan_path = bundle.join("evidence/artifact-plan.v1");
    let plan: ArtifactPlanV1 =
        serde_json::from_slice(&file_sha256(&plan_path)?.1).map_err(|error| {
            ForgeError::VerusOutput {
                detail: format!("invalid bound ArtifactPlanV1: {error}"),
            }
        })?;
    let expected_plan_schema = if composition_receipt {
        COMPOSITION_PLAN_SCHEMA
    } else {
        PLAN_SCHEMA
    };
    if plan.schema != expected_plan_schema || plan.canonical_sha256() != receipt.binding.plan_sha256
    {
        return Err(ForgeError::VerusOutput {
            detail: "bound ArtifactPlanV1 failed its canonical digest".to_string(),
        });
    }
    if plan.exports != receipt.binding.exports
        || plan.expected_verus_source_sha256 != receipt.binding.verus_source_sha256
        || plan.raw_source_sha256 != receipt.binding.raw_source_sha256
        || plan.parsed_program_sha256 != receipt.binding.parsed_program_sha256
    {
        return Err(ForgeError::VerusOutput {
            detail: "receipt binding disagrees with its bound ArtifactPlanV1".to_string(),
        });
    }
    match (&plan.composition, &receipt.binding.composition) {
        (None, None) if !composition_receipt => {}
        (Some(composition), Some(binding)) if composition_receipt => {
            if binding.lowered_thermite_sha256 != composition.lowered_thermite_sha256
                || binding.direct_verus_set_sha256
                    != canonical_shell_set_sha256(&composition.shell_modules)
                || binding.inventory_sha256
                    != canonical_composition_inventory_sha256(&composition.inventory)
                || binding.combined_source_sha256 != composition.combined_source_sha256
                || composition.combined_source_sha256 != plan.expected_verus_source_sha256
            {
                return Err(ForgeError::VerusOutput {
                    detail: "composition receipt binding disagrees with its combined artifact plan"
                        .to_string(),
                });
            }
        }
        _ => {
            return Err(ForgeError::VerusOutput {
                detail: "receipt schema and composition binding disagree".to_string(),
            })
        }
    }
    if receipt.binding.assurance != "L3"
        || receipt.binding.scope != "end_to_end"
        || receipt.binding.crate_name != plan.crate_name
        || receipt.binding.target != plan.target
    {
        return Err(ForgeError::VerusOutput {
            detail: "receipt headline, crate, or target disagrees with ArtifactPlanV1".to_string(),
        });
    }

    let raw_source = file_sha256(&bundle.join("evidence/input.th"))?.1;
    if sha256(&raw_source) != plan.raw_source_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "bound Thermite input disagrees with ArtifactPlanV1".to_string(),
        });
    }
    let bound_package = crate::thermite_package::load_evidence(bundle, &raw_source)?;
    if plan.thermite_package != bound_package.as_ref().map(thermite_package_plan) {
        return Err(ForgeError::VerusOutput {
            detail: "bound Thermite package inventory disagrees with ArtifactPlanV1".to_string(),
        });
    }
    let source_text =
        std::str::from_utf8(&raw_source).map_err(|error| ForgeError::VerusOutput {
            detail: format!("bound Thermite input is not UTF-8: {error}"),
        })?;
    let parsed = thermite_syntax::parse(source_text);
    if !parsed.is_clean() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "bound Thermite input no longer parses cleanly: {:?}",
                parsed.errors
            ),
        });
    }
    thermite_spec::validate(&parsed.program).map_err(|errors| ForgeError::VerusOutput {
        detail: format!("bound Thermite input fails spec validation: {errors:?}"),
    })?;
    thermite_lower::check_effects(&parsed.program).map_err(|errors| ForgeError::VerusOutput {
        detail: format!("bound Thermite input fails effect validation: {errors:?}"),
    })?;
    if normalized_program_sha256(&parsed.program) != plan.parsed_program_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "bound parsed-program digest disagrees with ArtifactPlanV1".to_string(),
        });
    }
    let link_roots: Vec<String> = plan
        .exports
        .iter()
        .map(|export| export.thermite_name.clone())
        .collect();
    let roots = plan_roots(&plan);
    let closure = closure::verified_closure(&parsed.program, &roots).map_err(|error| {
        ForgeError::VerusOutput {
            detail: format!("bound closure is incomplete: {error}"),
        }
    })?;
    let direct_frozen_boundaries = plan.composition.is_some()
        && matches!(
            plan.target,
            VerifiedTarget::Kernel | VerifiedTarget::KernelUefi
        );
    if let Some(detail) = strict_source_checks(
        &parsed.program,
        &closure,
        plan.target,
        direct_frozen_boundaries,
    ) {
        return Err(ForgeError::VerusOutput {
            detail: format!("bound closure violates strict policy: {detail}"),
        });
    }
    let exports = plan_exports(
        &parsed.program,
        &link_roots,
        &plan.crate_name,
        plan.target,
        &plan.target_triple,
        &plan.target_pointer_width,
        &plan.target_endian,
    )
    .map_err(|detail| ForgeError::VerusOutput {
        detail: format!("bound export plan is invalid: {detail}"),
    })?;
    if exports != plan.exports {
        return Err(ForgeError::VerusOutput {
            detail: "bound export rows do not match independently reconstructed ABI rows"
                .to_string(),
        });
    }
    let subprogram = closure_program(&parsed.program, &closure);
    let (independently_emitted, reconstructed_plan) = if composition_receipt {
        let (reconstructed, lowered, combined, reconstructed_closure, reconstructed_exports) =
            composition::reconstruct_plan(
                &parsed.program,
                &raw_source,
                bound_package.as_ref(),
                &plan,
                bundle,
            )?;
        let lowered_bytes = file_sha256(&bundle.join("evidence/lowered-thermite.verus.rs"))?.1;
        if lowered.as_bytes() != lowered_bytes
            || plan.composition.as_ref().is_none_or(|composition| {
                sha256(&lowered_bytes) != composition.lowered_thermite_sha256
            })
            || reconstructed_closure != closure
            || reconstructed_exports != exports
        {
            return Err(ForgeError::VerusOutput {
                detail: "bound Thermite lowering or composition closure failed independent reconstruction"
                    .to_string(),
            });
        }
        (combined, reconstructed)
    } else {
        let lower_exports: Vec<L3Export> = exports
            .iter()
            .map(|export| L3Export {
                source_name: export.thermite_name.clone(),
                public_name: export.public_name.clone(),
                wrapped: export.wrapped,
                visibility: L3ExportVisibility::Public,
            })
            .collect();
        let lower_target = match plan.target {
            VerifiedTarget::Std => L3LibraryTarget::Std,
            VerifiedTarget::Kernel | VerifiedTarget::KernelUefi => L3LibraryTarget::Kernel,
        };
        let emitted = thermite_lower::lower_l3_library(&subprogram, &lower_exports, lower_target)
            .map_err(ForgeError::Lower)?;
        let reconstructed = make_plan(PlanInput {
            raw_source: &raw_source,
            package: bound_package.as_ref(),
            program: &parsed.program,
            selected_program: &subprogram,
            closure: &closure,
            exports: &exports,
            crate_name: &plan.crate_name,
            target: plan.target,
            target_triple: &plan.target_triple,
            target_pointer_width: &plan.target_pointer_width,
            target_endian: &plan.target_endian,
            verus_source: &emitted,
        });
        (emitted, reconstructed)
    };
    let bound_verus_source = String::from_utf8(
        file_sha256(&bundle.join("evidence/source.verus.rs"))?.1,
    )
    .map_err(|error| ForgeError::VerusOutput {
        detail: format!("bound canonical Verus source is not UTF-8: {error}"),
    })?;
    if bound_verus_source != independently_emitted
        || sha256(bound_verus_source.as_bytes()) != plan.expected_verus_source_sha256
        || forbidden_emission(&bound_verus_source).is_some()
    {
        return Err(ForgeError::VerusOutput {
            detail:
                "bound canonical Verus source is not the strict source reconstructed from the plan"
                    .to_string(),
        });
    }
    if reconstructed_plan != plan {
        return Err(ForgeError::VerusOutput {
            detail: "ArtifactPlanV1 does not equal the independently reconstructed frozen plan"
                .to_string(),
        });
    }

    let certificate_bytes = file_sha256(&bundle.join("evidence/certificates.json"))?.1;
    if sha256(&certificate_bytes) != receipt.binding.certificate_set_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "certificate-set semantic digest mismatch".to_string(),
        });
    }
    let certificates: Vec<Certificate> =
        serde_json::from_slice(&certificate_bytes).map_err(|error| ForgeError::VerusOutput {
            detail: format!("invalid bound certificate set: {error}"),
        })?;
    if let Some(detail) = reject_certificates(
        &certificates,
        &closure,
        &parsed.program,
        direct_frozen_boundaries,
    ) {
        return Err(ForgeError::VerusOutput {
            detail: format!("bound certificate set fails strict L3 policy: {detail}"),
        });
    }
    let direct_refined_boundary_names: BTreeSet<String> = plan
        .composition
        .as_ref()
        .map(|composition| {
            composition
                .direct_refined_boundaries
                .iter()
                .map(|boundary| boundary.thermite_name.clone())
                .collect()
        })
        .unwrap_or_default();
    let reconstructed_assurance = assurance_aggregate(
        &certificates,
        &closure,
        &exports,
        &direct_refined_boundary_names,
    )?;
    if receipt.binding.assurance_aggregate != reconstructed_assurance
        || reconstructed_assurance.headline != "L3"
        || reconstructed_assurance.cap != "L3"
        || reconstructed_assurance.scope != "end_to_end"
        || !matches!(
            reconstructed_assurance.minimum_reachable.as_str(),
            "L3" | "L4"
        )
    {
        return Err(ForgeError::VerusOutput {
            detail: "receipt assurance aggregate is not the minimum of the complete L3 closure"
                .to_string(),
        });
    }

    let tv_bytes = file_sha256(&bundle.join("evidence/translation-validation.json"))?.1;
    if sha256(&tv_bytes) != receipt.binding.translation_validation_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "translation-validation semantic digest mismatch".to_string(),
        });
    }
    let tv: TranslationValidationEvidence =
        serde_json::from_slice(&tv_bytes).map_err(|error| ForgeError::VerusOutput {
            detail: format!("invalid bound translation-validation evidence: {error}"),
        })?;
    if tv.seed != DEFAULT_SOLVER_SEED || tv.rlimit != DEFAULT_RLIMIT.to_string() {
        return Err(ForgeError::VerusOutput {
            detail: "bound TV evidence uses a noncanonical seed or resource limit".to_string(),
        });
    }
    if let Some(detail) = reject_translation_validation(&tv, &parsed.program, &closure, &exports) {
        return Err(ForgeError::VerusOutput {
            detail: format!("bound TV evidence fails strict completeness: {detail}"),
        });
    }

    let verus_bytes = file_sha256(&bundle.join("evidence/verus-result.json"))?.1;
    if sha256(&verus_bytes) != receipt.binding.whole_crate_verus_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "whole-crate Verus semantic digest mismatch".to_string(),
        });
    }
    let verus: VerusEvidence =
        serde_json::from_slice(&verus_bytes).map_err(|error| ForgeError::VerusOutput {
            detail: format!("invalid bound whole-crate Verus evidence: {error}"),
        })?;
    if !verus.success
        || verus.errors != Some(0)
        || verus.args != plan.expected_verus_args
        || plan.expected_verus_args != expected_verus_args(&plan.crate_name, plan.target)
        || verus.source_relative_path != format!("{}.rs", plan.crate_name)
        || verus.source_sha256_before != plan.expected_verus_source_sha256
        || verus.source_sha256_after != plan.expected_verus_source_sha256
        || parse_verus_summary(&verus.stdout) != (true, Some(0))
    {
        return Err(ForgeError::VerusOutput {
            detail: "bound whole-crate result does not prove strict no-cheating codegen of the canonical source"
                .to_string(),
        });
    }
    validate_verus_scope_evidence(&plan, &verus)?;

    let toolchain_bytes = file_sha256(&bundle.join("evidence/toolchain.json"))?.1;
    if sha256(&toolchain_bytes) != receipt.binding.toolchain_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "toolchain semantic digest mismatch".to_string(),
        });
    }
    let toolchain: ToolchainEvidence =
        serde_json::from_slice(&toolchain_bytes).map_err(|error| ForgeError::VerusOutput {
            detail: format!("invalid bound toolchain evidence: {error}"),
        })?;
    let environment_keys: BTreeSet<&str> =
        toolchain.environment.keys().map(String::as_str).collect();
    let rustup_parent = Path::new(&toolchain.rustup_path)
        .parent()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    validate_codegen_evidence(&toolchain).map_err(|detail| ForgeError::VerusOutput {
        detail: format!("bound artifact-codegen toolchain is invalid: {detail}"),
    })?;
    let codegen_host = rustc_version_field(&toolchain.artifact_codegen.rustc_version, "host: ")?;
    let target_triple_matches_plan = match plan.target {
        VerifiedTarget::KernelUefi => toolchain.target_triple == "x86_64-unknown-uefi",
        VerifiedTarget::Std | VerifiedTarget::Kernel => toolchain.target_triple == codegen_host,
    };
    if toolchain.source_date_epoch != SOURCE_DATE_EPOCH
        || toolchain.forge_version != env!("CARGO_PKG_VERSION")
        || toolchain.target_triple != plan.target_triple
        || toolchain.target_pointer_width != plan.target_pointer_width
        || toolchain.target_endian != plan.target_endian
        || environment_keys
            != BTreeSet::from([
                "HOME",
                "PATH",
                "RUSTUP_HOME",
                "RUSTUP_TOOLCHAIN",
                "SOURCE_DATE_EPOCH",
            ])
        || toolchain
            .environment
            .get("SOURCE_DATE_EPOCH")
            .map(String::as_str)
            != Some(SOURCE_DATE_EPOCH)
        || toolchain
            .environment
            .get("PATH")
            .is_none_or(|path| path.split(':').next() != Some(rustup_parent.as_str()))
        || toolchain
            .environment
            .get("HOME")
            .is_none_or(String::is_empty)
        || toolchain
            .environment
            .get("RUSTUP_HOME")
            .is_none_or(String::is_empty)
        || toolchain
            .environment
            .get("RUSTUP_TOOLCHAIN")
            .map(String::as_str)
            != Some(toolchain.artifact_codegen.rustup_toolchain.as_str())
        || verus.codegen_toolchain_sha256 != toolchain.artifact_codegen.canonical_identity_sha256()
        || !target_triple_matches_plan
    {
        return Err(ForgeError::VerusOutput {
            detail: "bound toolchain policy or environment whitelist is invalid".to_string(),
        });
    }
    if file_sha256(&bundle.join("evidence/Cargo.lock"))?.2 != toolchain.cargo_lock_sha256 {
        return Err(ForgeError::VerusOutput {
            detail: "copied Cargo.lock has the wrong toolchain digest".to_string(),
        });
    }
    let dependency_names: BTreeSet<&str> = toolchain
        .link_dependencies
        .iter()
        .map(|dependency| dependency.name.as_str())
        .collect();
    if dependency_names
        != BTreeSet::from([
            "libverus_builtin.rlib",
            "libverus_builtin_macros.so",
            "libverus_state_machines_macros.so",
            "libvstd.rlib",
        ])
        || dependency_names.len() != toolchain.link_dependencies.len()
    {
        return Err(ForgeError::VerusOutput {
            detail: "bound Verus link-dependency inventory is incomplete or duplicated".to_string(),
        });
    }
    for dependency in &toolchain.link_dependencies {
        let copied = bundle.join("artifact/deps").join(&dependency.name);
        if file_sha256(&copied)?.2 != dependency.sha256 {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "copied link dependency `{}` has the wrong digest",
                    dependency.name
                ),
            });
        }
    }
    match (plan.target, toolchain.kernel_vstd_model.as_ref()) {
        (VerifiedTarget::Kernel | VerifiedTarget::KernelUefi, Some(model)) => {
            let vstd_dependency = toolchain
                .link_dependencies
                .iter()
                .find(|dependency| dependency.name == "libvstd.rlib")
                .ok_or_else(|| ForgeError::VerusOutput {
                    detail: "kernel model has no bound libvstd.rlib".to_string(),
                })?;
            if model.link_source_name != KERNEL_VSTD_LINK_SOURCE_NAME
                || model.target_triple != plan.target_triple
                || model.link_build_args != kernel_vstd_link_build_args(plan.target)
                || model.link_rlib_sha256 != vstd_dependency.sha256
                || vstd_dependency.source_path != "<VERUS>/vstd/vstd.rs"
                || model.source_file_count == 0
                || model.source_total_bytes == 0
                || model.source_sha256.len() != 64
                || model.vir_sha256.len() != 64
                || model.link_source_sha256.len() != 64
                || model.link_prelude_sha256.len() != 64
            {
                return Err(ForgeError::VerusOutput {
                    detail: "bound kernel vstd model identity is malformed or inconsistent"
                        .to_string(),
                });
            }
            match plan.target {
                VerifiedTarget::KernelUefi
                    if model.target_builtin_source_path.is_none()
                        || model
                            .target_builtin_source_sha256
                            .as_ref()
                            .is_none_or(|digest| digest.len() != 64)
                        || model.target_builtin_build_args != kernel_uefi_builtin_build_args() =>
                {
                    return Err(ForgeError::VerusOutput {
                        detail: "bound UEFI verus_builtin source/build identity is incomplete"
                            .to_string(),
                    });
                }
                VerifiedTarget::Kernel
                    if model.target_builtin_source_path.is_some()
                        || model.target_builtin_source_sha256.is_some()
                        || !model.target_builtin_build_args.is_empty() =>
                {
                    return Err(ForgeError::VerusOutput {
                        detail: "host kernel model unexpectedly binds a target builtin build"
                            .to_string(),
                    });
                }
                _ => {}
            }
            let vstd_evidence = bundle
                .join("evidence")
                .join(KERNEL_VSTD_SOURCE_EVIDENCE_DIR);
            if directory_sha256_named(&vstd_evidence, "thermite.kernel-vstd-source-tree.v1")?
                != (
                    model.source_file_count,
                    model.source_total_bytes,
                    model.source_sha256.clone(),
                )
                || file_sha256(&bundle.join("evidence").join(KERNEL_VSTD_VIR_EVIDENCE_NAME))?.2
                    != model.vir_sha256
            {
                return Err(ForgeError::VerusOutput {
                    detail: "bound kernel vstd source/VIR closure has the wrong identity"
                        .to_string(),
                });
            }
            let link_evidence = bundle.join("evidence").join(KERNEL_VSTD_LINK_EVIDENCE_DIR);
            if file_sha256(&link_evidence.join(KERNEL_VSTD_LINK_SOURCE_NAME))?.2
                != model.link_source_sha256
                || file_sha256(&link_evidence.join(KERNEL_VSTD_LINK_PRELUDE_NAME))?.2
                    != model.link_prelude_sha256
            {
                return Err(ForgeError::VerusOutput {
                    detail: "bound kernel vstd selected link sources have the wrong identity"
                        .to_string(),
                });
            }
            if let Some(expected) = &model.target_builtin_source_sha256 {
                if file_sha256(&bundle.join("evidence").join(KERNEL_BUILTIN_EVIDENCE_NAME))?.2
                    != *expected
                {
                    return Err(ForgeError::VerusOutput {
                        detail: "bound kernel verus_builtin source has the wrong identity"
                            .to_string(),
                    });
                }
            }
        }
        (VerifiedTarget::Std, None) => {}
        _ => {
            return Err(ForgeError::VerusOutput {
                detail: "kernel vstd model presence disagrees with the verified target".to_string(),
            });
        }
    }
    let artifact_file = receipt
        .binding
        .files
        .iter()
        .find(|file| file.path == receipt.binding.artifact.path)
        .ok_or_else(|| ForgeError::VerusOutput {
            detail: "receipt binds no rlib artifact".to_string(),
        })?;
    let artifact = &receipt.binding.artifact;
    if artifact.kind != "rlib"
        || artifact.path != format!("artifact/lib{}.rlib", plan.crate_name)
        || artifact.length != artifact_file.length
        || artifact.sha256 != artifact_file.sha256
    {
        return Err(ForgeError::VerusOutput {
            detail: "first-class artifact binding disagrees with the file inventory or plan"
                .to_string(),
        });
    }

    if replay {
        let current_verus = resolve_executable(std::env::var_os("VERUS_BIN").as_deref(), "verus")?;
        let current_rustup = resolve_executable(None, "rustup")?;
        let current_verus_version = command_text(
            Command::new(&current_verus).arg("--version"),
            "replay verus --version",
        )?;
        let current_codegen =
            collect_codegen_rustc(&current_verus_version, &current_rustup, plan.target)?;
        if file_sha256(&current_verus)?.2 != toolchain.verus_sha256
            || current_verus_version != toolchain.verus_version
            || !current_codegen.same_identity(&toolchain.artifact_codegen)
        {
            return Err(ForgeError::VerusOutput {
                detail: "replay Verus or its selected Rust/LLVM codegen closure does not match the bound toolchain"
                    .to_string(),
            });
        }
        let current_forge = std::env::current_exe().map_err(|source| ForgeError::Io {
            path: "current forge executable".to_string(),
            source,
        })?;
        let current_z3 = current_verus
            .parent()
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: "replay Verus binary has no installation directory".to_string(),
            })?
            .join("z3");
        if file_sha256(&current_forge)?.2 != toolchain.forge_executable_sha256
            || file_sha256(&current_rustup)?.2 != toolchain.rustup_sha256
            || command_text(
                Command::new(&current_rustup).arg("--version"),
                "rustup --version",
            )? != toolchain.rustup_version
            || file_sha256(&current_z3)?.2 != toolchain.z3_sha256
            || command_text(
                Command::new(&current_z3).arg("--version"),
                "replay z3 --version",
            )? != toolchain.z3_version
        {
            return Err(ForgeError::VerusOutput {
                detail: "replay Forge, rustup, or Z3 does not match the bound toolchain"
                    .to_string(),
            });
        }
        let source = String::from_utf8(file_sha256(&bundle.join("evidence/source.verus.rs"))?.1)
            .map_err(|error| ForgeError::VerusOutput {
                detail: format!("bound Verus source is not UTF-8: {error}"),
            })?;
        let replay_environment = closed_verus_environment(
            &current_rustup,
            &toolchain.artifact_codegen.rustup_toolchain,
        )?;
        let replay_kernel_vstd = if matches!(
            plan.target,
            VerifiedTarget::Kernel | VerifiedTarget::KernelUefi
        ) {
            let current_verus_dir =
                current_verus
                    .parent()
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "replay Verus binary has no installation directory".to_string(),
                    })?;
            let replay_vstd_source = bundle
                .join("evidence")
                .join(KERNEL_VSTD_SOURCE_EVIDENCE_DIR);
            let replay_vstd_vir = bundle.join("evidence").join(KERNEL_VSTD_VIR_EVIDENCE_NAME);
            let replay_builtin_source = bundle.join("evidence").join(KERNEL_BUILTIN_EVIDENCE_NAME);
            let rebuilt = build_kernel_vstd_link(KernelVstdLinkRequest {
                verus: &current_verus,
                verus_dir: current_verus_dir,
                environment: &replay_environment,
                target: plan.target,
                codegen: &current_codegen,
                source_root_override: Some(&replay_vstd_source),
                vir_override: Some(&replay_vstd_vir),
                builtin_source_override: (plan.target == VerifiedTarget::KernelUefi)
                    .then_some(replay_builtin_source.as_path()),
            })?;
            if Some(&rebuilt.2) != toolchain.kernel_vstd_model.as_ref() {
                let expected = serde_json::to_string(&toolchain.kernel_vstd_model)
                    .unwrap_or_else(|_| "<unserializable>".to_string());
                let actual = serde_json::to_string(&rebuilt.2)
                    .unwrap_or_else(|_| "<unserializable>".to_string());
                let expected_dependencies = serde_json::to_string(
                    &toolchain
                        .link_dependencies
                        .iter()
                        .filter(|dependency| {
                            dependency.name == "libvstd.rlib"
                                || dependency.name == "libverus_builtin.rlib"
                        })
                        .collect::<Vec<_>>(),
                )
                .unwrap_or_else(|_| "<unserializable>".to_string());
                let actual_dependencies = serde_json::to_string(&rebuilt.1)
                    .unwrap_or_else(|_| "<unserializable>".to_string());
                return Err(ForgeError::VerusOutput {
                    detail: format!(
                        "replay kernel vstd model/source/link identity does not match the receipt; expected={expected}; actual={actual}; expected_dependencies={expected_dependencies}; actual_dependencies={actual_dependencies}"
                    ),
                });
            }
            Some(rebuilt)
        } else {
            None
        };
        let replay_kernel_vstd_path = replay_kernel_vstd
            .as_ref()
            .map(|(scratch, _, _)| scratch.path.join("libvstd.rlib"));
        let replay_vstd_vir_path = matches!(
            plan.target,
            VerifiedTarget::Kernel | VerifiedTarget::KernelUefi
        )
        .then(|| bundle.join("evidence").join(KERNEL_VSTD_VIR_EVIDENCE_NAME));
        let replay_builtin_path = replay_kernel_vstd.as_ref().and_then(|(scratch, _, _)| {
            (plan.target == VerifiedTarget::KernelUefi)
                .then(|| scratch.path.join("libverus_builtin.rlib"))
        });
        let replay_dependencies = KernelCompileDependencies {
            vstd_vir: replay_vstd_vir_path.as_deref(),
            vstd_rlib: replay_kernel_vstd_path.as_deref(),
            verus_builtin_rlib: replay_builtin_path.as_deref(),
        };
        let mut compiled = compile_verus_source(
            &plan.crate_name,
            &source,
            plan.target,
            current_verus.to_string_lossy().as_ref(),
            &replay_environment,
            &current_codegen.canonical_identity_sha256(),
            replay_dependencies,
        )?;
        if !compiled.evidence.success || sha256(&compiled.artifact) != artifact.sha256 {
            return Err(ForgeError::VerusOutput {
                detail: "replay did not reproduce the bound artifact digest".to_string(),
            });
        }
        if let Some(composition) = &plan.composition {
            compiled.evidence.scopes = collect_composition_scope_evidence(
                &plan.crate_name,
                &source,
                plan.target,
                &composition.shell_modules,
                current_verus.to_string_lossy().as_ref(),
                &replay_environment,
                &current_codegen.canonical_identity_sha256(),
                replay_dependencies,
            )?;
            validate_verus_scope_evidence(&plan, &compiled.evidence)?;
            if compiled.evidence.scopes != verus.scopes {
                return Err(ForgeError::VerusOutput {
                    detail: "replay did not reproduce scoped Verus obligation attribution"
                        .to_string(),
                });
            }
        }
    }

    Ok(VerifyBuildReport {
        bundle: bundle.to_path_buf(),
        binding_sha256: receipt.binding_sha256,
        replayed: replay,
        artifact_sha256: artifact.sha256.clone(),
    })
}

fn validate_relative_path(path: &str) -> Result<(), ForgeError> {
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ForgeError::VerusOutput {
            detail: format!("receipt contains unsafe bundle-relative path `{path}`"),
        });
    }
    Ok(())
}

fn sync_tree(root: &Path) -> Result<(), ForgeError> {
    for entry in fs::read_dir(root).map_err(|source| ForgeError::Io {
        path: root.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| ForgeError::Io {
            path: root.display().to_string(),
            source,
        })?;
        if entry
            .file_type()
            .map_err(|source| ForgeError::Io {
                path: entry.path().display().to_string(),
                source,
            })?
            .is_dir()
        {
            sync_tree(&entry.path())?;
        }
    }
    sync_dir(root)
}

fn sync_dir(path: &Path) -> Result<(), ForgeError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| ForgeError::Io {
            path: path.display().to_string(),
            source,
        })
}

struct ScratchTree {
    path: PathBuf,
    armed: std::cell::Cell<bool>,
}

impl ScratchTree {
    fn new_in_temp(stem: &str) -> Result<Self, ForgeError> {
        let path = unique_path(&std::env::temp_dir(), stem);
        Self::create(path)
    }

    fn new_sibling(destination: &Path) -> Result<Self, ForgeError> {
        let parent = destination.parent().unwrap_or(Path::new("."));
        let stem = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("verified");
        Self::create(unique_path(parent, &format!(".{stem}.stage")))
    }

    fn create(path: PathBuf) -> Result<Self, ForgeError> {
        fs::create_dir(&path).map_err(|source| ForgeError::Io {
            path: path.display().to_string(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(|source| {
                ForgeError::Io {
                    path: path.display().to_string(),
                    source,
                }
            })?;
        }
        Ok(Self {
            path,
            armed: std::cell::Cell::new(true),
        })
    }

    fn disarm(&self) {
        self.armed.set(false);
    }
}

impl Drop for ScratchTree {
    fn drop(&mut self) {
        if self.armed.get() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn unique_path(parent: &Path, stem: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = OsString::from(stem);
    name.push(format!(".{}.{}", std::process::id(), n));
    parent.join(name)
}

fn test_fault(name: &str) -> bool {
    cfg!(debug_assertions)
        && std::env::var("THERMITE_L3_TEST_FAULT").is_ok_and(|value| value == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Program {
        let parsed = thermite_syntax::parse(source);
        assert!(parsed.is_clean(), "{:?}", parsed.errors);
        parsed.program
    }

    fn sample_verus_evidence(errors: Option<u64>) -> VerusEvidence {
        VerusEvidence {
            args: Vec::new(),
            source_relative_path: "sample.rs".to_string(),
            source_sha256_before: "a".repeat(64),
            source_sha256_after: "a".repeat(64),
            codegen_toolchain_sha256: "b".repeat(64),
            success: false,
            errors,
            stdout: String::new(),
            stderr: "frontend rejected the crate".to_string(),
            scopes: Vec::new(),
        }
    }

    #[test]
    fn verus_summary_distinguishes_reported_and_unknown_error_counts() {
        assert_eq!(
            parse_verus_summary(
                r#"{"verification-results":{"success":true,"verified":2,"errors":0}}"#,
            ),
            (true, Some(0)),
        );
        assert_eq!(
            parse_verus_summary(
                r#"{"verification-results":{"success":false,"verified":1,"errors":3}}"#,
            ),
            (false, Some(3)),
        );
        assert_eq!(
            parse_verus_summary(
                r#"{"verification-results":{"success":false,"encountered-vir-error":true}}"#,
            ),
            (false, None),
        );
        assert_eq!(parse_verus_summary("not json"), (false, None));
        assert_eq!(parse_verus_summary("{}"), (false, None));
    }

    #[test]
    fn verus_failure_detail_claims_only_structured_counts() {
        let known = verus_failure_detail("strict Verus failed", &sample_verus_evidence(Some(3)));
        assert_eq!(
            known,
            "strict Verus failed (errors=3): frontend rejected the crate"
        );

        let unknown = verus_failure_detail("strict Verus failed", &sample_verus_evidence(None));
        assert_eq!(unknown, "strict Verus failed: frontend rejected the crate");
        assert!(!unknown.contains(&u64::MAX.to_string()));
        assert!(!unknown.contains("errors="));
    }

    #[test]
    fn verus_evidence_keeps_numeric_success_compatibility_and_defaults_to_unknown() {
        let evidence = sample_verus_evidence(Some(0));
        let mut encoded = serde_json::to_value(&evidence).unwrap();
        assert_eq!(encoded["errors"], serde_json::json!(0));
        encoded.as_object_mut().unwrap().remove("errors");
        let decoded: VerusEvidence = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.errors, None);
    }

    fn sample_codegen(root: &str) -> CodegenRustcEvidence {
        CodegenRustcEvidence {
            selection: "verus --version Toolchain".to_string(),
            rustup_toolchain: "1.95.0-x86_64-unknown-linux-gnu".to_string(),
            rustc_path: format!("{root}/bin/rustc"),
            rustc_sha256: "1".repeat(64),
            rustc_version: "rustc 1.95.0\nbinary: rustc\ncommit-hash: abc\nrelease: 1.95.0\nhost: x86_64-unknown-linux-gnu\nLLVM version: 21.1.8".to_string(),
            rustc_release: "1.95.0".to_string(),
            rustc_commit_hash: "abc".to_string(),
            sysroot: root.to_string(),
            rustc_component_manifest_path: format!("{root}/lib/rustlib/manifest-rustc"),
            rustc_component_manifest_sha256: "2".repeat(64),
            rust_std_component_manifest_path: format!("{root}/lib/rustlib/manifest-rust-std"),
            rust_std_component_manifest_sha256: "3".repeat(64),
            rustc_driver_path: format!("{root}/lib/librustc_driver.so"),
            rustc_driver_sha256: "4".repeat(64),
            llvm_version: "21.1.8".to_string(),
            llvm_library_path: format!("{root}/lib/libLLVM.so"),
            llvm_library_sha256: "5".repeat(64),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            target_pointer_width: "64".to_string(),
            target_endian: "little".to_string(),
            target_libdir: format!("{root}/lib/rustlib/x86_64-unknown-linux-gnu/lib"),
            target_libdir_sha256: "6".repeat(64),
            target_libdir_file_count: 62,
            target_libdir_total_bytes: 166_568_014,
            linker_identity: "rlib: no final linker invoked by artifact codegen".to_string(),
        }
    }

    #[test]
    fn canonical_plan_hash_is_json_whitespace_independent() {
        let plan = ArtifactPlanV1 {
            schema: PLAN_SCHEMA.to_string(),
            raw_source_sha256: "a".repeat(64),
            parsed_program_sha256: "b".repeat(64),
            crate_name: "demo".to_string(),
            target: VerifiedTarget::Std,
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            target_pointer_width: "64".to_string(),
            target_endian: "little".to_string(),
            crate_type: "rlib".to_string(),
            panic_strategy: "abort".to_string(),
            expected_verus_args: Vec::new(),
            exports: Vec::new(),
            closure_nodes: Vec::new(),
            closure_edges: Vec::new(),
            item_dispositions: Vec::new(),
            strict_gates: STRICT_GATES.iter().map(|s| (*s).to_string()).collect(),
            expected_tv_inventory: Vec::new(),
            expected_verus_source_sha256: "c".repeat(64),
            thermite_package: None,
            composition: None,
        };
        let compact = serde_json::to_string(&plan).unwrap();
        let pretty = serde_json::to_string_pretty(&plan).unwrap();
        let a: ArtifactPlanV1 = serde_json::from_str(&compact).unwrap();
        let b: ArtifactPlanV1 = serde_json::from_str(&pretty).unwrap();
        assert_eq!(a.canonical_sha256(), b.canonical_sha256());
    }

    #[test]
    fn unsafe_receipt_paths_are_rejected() {
        assert!(validate_relative_path("evidence/input.th").is_ok());
        assert!(validate_relative_path("../input.th").is_err());
        assert!(validate_relative_path("/tmp/input.th").is_err());
    }

    #[test]
    fn l3_orchestrator_has_no_l1_lowering_call() {
        let source = include_str!("verified_build.rs");
        let l1_call = ["thermite_lower::", "lower_l1", "("].concat();
        let runtime_check = ["thermite_", "check!", "("].concat();
        assert!(!source.contains(&l1_call));
        assert!(!source.contains(&runtime_check));
    }

    #[test]
    fn export_plan_is_explicit_private_by_default_and_wraps_nontrivial_req() {
        let program = parse(
            "fn direct(x: u64) -> u64 req true ens result == x fx pure { x } \
             fn guarded(x: u64) -> u64 req x < 100 ens result == x fx pure { x } \
             fn hidden(x: u64) -> u64 req true ens result == x fx pure { x }",
        );
        let exports = plan_exports(
            &program,
            &["guarded".to_string(), "direct".to_string()],
            "demo",
            VerifiedTarget::Std,
            "x86_64-unknown-linux-gnu",
            "64",
            "little",
        )
        .unwrap();
        assert_eq!(exports.len(), 2);
        assert_eq!(exports[0].public_name, "direct");
        assert!(!exports[0].wrapped);
        assert_eq!(exports[1].public_name, "thermite_export_guarded_v1");
        assert!(exports[1].wrapped);
        assert!(!exports
            .iter()
            .any(|export| export.thermite_name == "hidden"));
    }

    #[test]
    fn frozen_source_digest_detects_proof_preserving_mutation() {
        let original = "pub fn identity(x: u64) -> u64 { x }";
        let mutated = "pub fn identity(x: u64) -> u64 { x + 0 }";
        assert_ne!(sha256(original.as_bytes()), sha256(mutated.as_bytes()));
    }

    #[test]
    fn normalized_program_digest_ignores_only_source_presentation() {
        let compact = parse("fn id(x: u64) -> u64 req x < 10 ens result == 10 fx pure { 1_0 }");
        let presented_differently = parse(
            "\nfn id ( x : u64 ) -> u64\n  req x < 10\n  ens result == 10\n  fx pure\n{ 10 }\n",
        );
        let changed = parse("fn id(x: u64) -> u64 req x < 10 ens result == 11 fx pure { 10 }");
        assert_eq!(
            normalized_program_sha256(&compact),
            normalized_program_sha256(&presented_differently)
        );
        assert_ne!(
            normalized_program_sha256(&compact),
            normalized_program_sha256(&changed)
        );
    }

    #[test]
    fn canonical_binding_changes_for_every_assurance_component() {
        let base = ReceiptBindingV1 {
            schema: RECEIPT_SCHEMA.to_string(),
            assurance: "L3".to_string(),
            scope: "end_to_end".to_string(),
            plan_sha256: "a".repeat(64),
            raw_source_sha256: "b".repeat(64),
            parsed_program_sha256: "c".repeat(64),
            verus_source_sha256: "d".repeat(64),
            certificate_set_sha256: "e".repeat(64),
            translation_validation_sha256: "f".repeat(64),
            whole_crate_verus_sha256: "1".repeat(64),
            toolchain_sha256: "2".repeat(64),
            crate_name: "demo".to_string(),
            target: VerifiedTarget::Std,
            artifact: BoundArtifact {
                path: "artifact/libdemo.rlib".to_string(),
                kind: "rlib".to_string(),
                length: 1,
                sha256: "3".repeat(64),
            },
            assurance_aggregate: AssuranceAggregate {
                headline: "L3".to_string(),
                cap: "L3".to_string(),
                minimum_reachable: "L3".to_string(),
                scope: "end_to_end".to_string(),
                members: Vec::new(),
            },
            exports: Vec::new(),
            strict_gates: STRICT_GATES.iter().map(|s| (*s).to_string()).collect(),
            files: Vec::new(),
            composition: None,
        };
        let digest = base.canonical_sha256();
        for field in 0..11 {
            let mut changed = base.clone();
            match field {
                0 => changed.plan_sha256 = "9".repeat(64),
                1 => changed.raw_source_sha256 = "9".repeat(64),
                2 => changed.parsed_program_sha256 = "9".repeat(64),
                3 => changed.verus_source_sha256 = "9".repeat(64),
                4 => changed.certificate_set_sha256 = "9".repeat(64),
                5 => changed.translation_validation_sha256 = "9".repeat(64),
                6 => changed.whole_crate_verus_sha256 = "9".repeat(64),
                7 => changed.toolchain_sha256 = "9".repeat(64),
                8 => changed.crate_name = "other".to_string(),
                9 => changed.artifact.sha256 = "9".repeat(64),
                10 => changed.assurance_aggregate.minimum_reachable = "L2".to_string(),
                _ => unreachable!(),
            }
            assert_ne!(digest, changed.canonical_sha256());
        }
    }

    #[test]
    fn verus_toolchain_parser_is_unique_safe_and_fail_closed() {
        assert_eq!(
            parse_verus_toolchain(
                "Verus 0.2026.05.24\nToolchain: 1.95.0-x86_64-unknown-linux-gnu\n"
            )
            .unwrap(),
            "1.95.0-x86_64-unknown-linux-gnu"
        );
        assert!(parse_verus_toolchain("Verus without a toolchain").is_err());
        assert!(parse_verus_toolchain("Toolchain: ../nightly").is_err());
        assert!(parse_verus_toolchain("Toolchain: stable\nToolchain: nightly").is_err());
    }

    #[test]
    fn codegen_identity_ignores_install_prefix_and_binds_the_complete_closure() {
        let base = sample_codegen("/first/sysroot");
        let relocated = sample_codegen("/equivalent/prefix");
        assert!(base.same_identity(&relocated));

        let digest = base.canonical_identity_sha256();
        for field in 0..18 {
            let mut changed = base.clone();
            match field {
                0 => changed.selection.push_str(" changed"),
                1 => changed.rustup_toolchain.push_str("-other"),
                2 => changed.rustc_sha256 = "9".repeat(64),
                3 => changed.rustc_version.push_str("\nchanged"),
                4 => changed.rustc_release.push_str("-changed"),
                5 => changed.rustc_commit_hash.push_str("changed"),
                6 => changed.rustc_component_manifest_sha256 = "9".repeat(64),
                7 => changed.rust_std_component_manifest_sha256 = "9".repeat(64),
                8 => changed.rustc_driver_sha256 = "9".repeat(64),
                9 => changed.llvm_version.push_str("-changed"),
                10 => changed.llvm_library_sha256 = "9".repeat(64),
                11 => changed.target_triple.push_str("-changed"),
                12 => changed.target_pointer_width = "32".to_string(),
                13 => changed.target_endian = "big".to_string(),
                14 => changed.target_libdir_sha256 = "9".repeat(64),
                15 => changed.target_libdir_file_count += 1,
                16 => changed.target_libdir_total_bytes += 1,
                17 => changed.linker_identity.push_str(" changed"),
                _ => unreachable!(),
            }
            assert_ne!(digest, changed.canonical_identity_sha256(), "field {field}");
        }
    }
}
