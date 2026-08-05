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
use crate::thermite_package::{self, LoadedPackage};

mod composition;
mod primitive_registry;

const PLAN_SCHEMA: &str = "thermite.artifact-plan.v1";
const RECEIPT_SCHEMA: &str = "thermite.verified-build-receipt.v1";
const COMPOSITION_PLAN_SCHEMA: &str = "thermite.combined-artifact-plan.v1";
const COMPOSITION_RECEIPT_SCHEMA: &str = "thermite.verified-composition-receipt.v1";
const SOURCE_DATE_EPOCH: &str = "0";
const KERNEL_VSTD_LINK_SOURCE_NAME: &str = "kernel-vstd-link.rs";
const KERNEL_VSTD_LINK_SOURCE: &str = include_str!("kernel_vstd_link.rs");
const MACHINE_ATOMIC_MODEL_SOURCE_PATH: &str = "evidence/machine-models/pinned-vstd-atomic.rs";
const MACHINE_VSTD_RLIB_PATH: &str = "artifact/deps/libvstd.rlib";
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
    "frozen-primitive-registry-closure",
    "exact-boundary-refinement",
    "whole-crate-no-cheating",
    "verus-codegen",
    "cryptographic-binding",
];
const MACHINE_COMPOSITION_STRICT_GATES: &[&str] = &[
    "parse-spec-effects",
    "complete-to-machine-boundary-closure",
    "source-completeness",
    "no-escape-hatches-in-checked-layers",
    "termination",
    "l3-application-function-certificates",
    "contract-tv-complete",
    "exec-tv-complete",
    "body-loop-tv-complete",
    "total-export-wrappers",
    "rich-composition-visibility",
    "direct-verus-source-policy",
    "combined-source-inventory",
    "frozen-primitive-registry-closure",
    "exact-checked-wrapper-refinement",
    "explicit-residual-machine-assumptions",
    "whole-checked-crate-no-cheating",
    "verus-codegen",
    "cryptographic-binding",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifiedTarget {
    Std,
    Kernel,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub item_sha256: String,
    pub body_sha256: Option<String>,
    pub contract_sha256: Option<String>,
    pub effects_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackagePlanV1 {
    pub schema: String,
    pub name: String,
    pub manifest_sha256: String,
    pub source_map_sha256: String,
    pub roots: Vec<String>,
    pub modules: Vec<PlannedPackageModuleV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedPackageModuleV1 {
    pub name: String,
    pub path: String,
    pub imports: Vec<String>,
    pub length: u64,
    pub sha256: String,
    pub projection_source_start: u64,
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
    pub length: u64,
    pub sha256: String,
    pub items: Vec<PlannedShellItem>,
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
pub struct PlannedPrimitiveTargetV1 {
    pub target_triple: String,
    pub target_features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedPrimitiveEntryV1 {
    pub semantic_name: String,
    pub version: u64,
    pub required: bool,
    pub reachable: bool,
    pub thermite_name: String,
    pub semantic_address: String,
    pub boundary_target: String,
    pub signature: String,
    pub contract_sha256: String,
    pub effects_sha256: String,
    pub effects: Vec<String>,
    pub parameter_ownership: Vec<String>,
    pub result_ownership: String,
    pub implementation_shell: String,
    pub implementation_item: String,
    pub implementation_source_sha256: String,
    pub implementation_linkage: String,
    pub implementation_abi: String,
    pub implementation_symbol: String,
    pub alignment: u64,
    pub model: String,
    pub refinement: String,
    pub proof_obligations: Vec<String>,
    pub concurrency: String,
    pub memory_orderings: Vec<String>,
    pub failure: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_operation: Option<String>,
    #[serde(default)]
    pub residual_assumptions: Vec<String>,
}

fn default_primitive_proof_basis() -> String {
    "verus_builtins".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedPrimitiveCrateV1 {
    pub name: String,
    pub authored_source_path: String,
    pub authored_source_length: u64,
    pub authored_source_sha256: String,
    pub crate_source_path: String,
    pub crate_source_sha256: String,
    #[serde(default = "default_primitive_proof_basis")]
    pub proof_basis: String,
    pub items: Vec<PlannedShellItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedPrimitiveRegistryV1 {
    pub schema: String,
    pub path: String,
    pub length: u64,
    pub sha256: String,
    pub target: PlannedPrimitiveTargetV1,
    pub entries: Vec<PlannedPrimitiveEntryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionPlanV1 {
    pub schema: String,
    pub composition_exports: Vec<PlannedCompositionExport>,
    pub shell_modules: Vec<PlannedShellModule>,
    #[serde(default)]
    pub primitive_crates: Vec<PlannedPrimitiveCrateV1>,
    pub inventory: Vec<CompositionInventoryRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive_registry: Option<PlannedPrimitiveRegistryV1>,
    pub lowered_thermite_sha256: String,
    pub combined_source_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectVerusSource {
    plan: PlannedShellModule,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrimitiveCrateSource {
    plan: PlannedPrimitiveCrateV1,
    authored_bytes: Vec<u8>,
    crate_source: String,
}

fn canonical_shell_set_sha256(modules: &[PlannedShellModule]) -> String {
    let mut c = Canonical::new("thermite.direct-verus-source-set.v1");
    for module in modules {
        c.record("module", |c| {
            c.field("name", &module.name);
            c.field("path", &module.path);
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
    pub package: Option<PackagePlanV1>,
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
                c.field("source_module", node.source_module.as_deref().unwrap_or(""));
                c.field("source_path", node.source_path.as_deref().unwrap_or(""));
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
        if let Some(package) = &self.package {
            c.record("package", |c| {
                c.field("schema", &package.schema);
                c.field("name", &package.name);
                c.field("manifest_sha256", &package.manifest_sha256);
                c.field("source_map_sha256", &package.source_map_sha256);
                for root in &package.roots {
                    c.field("root", root);
                }
                for module in &package.modules {
                    c.record("module", |c| {
                        c.field("name", &module.name);
                        c.field("path", &module.path);
                        for import in &module.imports {
                            c.field("import", import);
                        }
                        c.field("length", &module.length.to_string());
                        c.field("sha256", &module.sha256);
                        c.field(
                            "projection_source_start",
                            &module.projection_source_start.to_string(),
                        );
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
                for primitive_crate in &composition.primitive_crates {
                    c.record("primitive_crate", |c| {
                        c.field("name", &primitive_crate.name);
                        c.field(
                            "authored_source_path",
                            &primitive_crate.authored_source_path,
                        );
                        c.field(
                            "authored_source_length",
                            &primitive_crate.authored_source_length.to_string(),
                        );
                        c.field(
                            "authored_source_sha256",
                            &primitive_crate.authored_source_sha256,
                        );
                        c.field("crate_source_path", &primitive_crate.crate_source_path);
                        c.field("crate_source_sha256", &primitive_crate.crate_source_sha256);
                        c.field("proof_basis", &primitive_crate.proof_basis);
                        for item in &primitive_crate.items {
                            c.record("item", |c| {
                                c.field("name", &item.name);
                                c.field("kind", &item.kind);
                                c.field("visibility", &item.visibility);
                            });
                        }
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
                if let Some(registry) = &composition.primitive_registry {
                    c.record("primitive_registry", |c| {
                        c.field("schema", &registry.schema);
                        c.field("path", &registry.path);
                        c.field("length", &registry.length.to_string());
                        c.field("sha256", &registry.sha256);
                        c.field("target_triple", &registry.target.target_triple);
                        for feature in &registry.target.target_features {
                            c.field("target_feature", feature);
                        }
                        for entry in &registry.entries {
                            c.record("entry", |c| {
                                c.field("semantic_name", &entry.semantic_name);
                                c.field("version", &entry.version.to_string());
                                c.field("required", if entry.required { "true" } else { "false" });
                                c.field(
                                    "reachable",
                                    if entry.reachable { "true" } else { "false" },
                                );
                                c.field("thermite_name", &entry.thermite_name);
                                c.field("semantic_address", &entry.semantic_address);
                                c.field("boundary_target", &entry.boundary_target);
                                c.field("signature", &entry.signature);
                                c.field("contract_sha256", &entry.contract_sha256);
                                c.field("effects_sha256", &entry.effects_sha256);
                                for effect in &entry.effects {
                                    c.field("effect", effect);
                                }
                                for ownership in &entry.parameter_ownership {
                                    c.field("parameter_ownership", ownership);
                                }
                                c.field("result_ownership", &entry.result_ownership);
                                c.field("implementation_shell", &entry.implementation_shell);
                                c.field("implementation_item", &entry.implementation_item);
                                c.field(
                                    "implementation_source_sha256",
                                    &entry.implementation_source_sha256,
                                );
                                c.field("implementation_linkage", &entry.implementation_linkage);
                                c.field("implementation_abi", &entry.implementation_abi);
                                c.field("implementation_symbol", &entry.implementation_symbol);
                                c.field("alignment", &entry.alignment.to_string());
                                c.field("model", &entry.model);
                                c.field("refinement", &entry.refinement);
                                for obligation in &entry.proof_obligations {
                                    c.field("proof_obligation", obligation);
                                }
                                c.field("concurrency", &entry.concurrency);
                                for ordering in &entry.memory_orderings {
                                    c.field("memory_ordering", ordering);
                                }
                                c.field("failure", &entry.failure);
                                c.field(
                                    "machine_family",
                                    entry.machine_family.as_deref().unwrap_or(""),
                                );
                                c.field(
                                    "machine_operation",
                                    entry.machine_operation.as_deref().unwrap_or(""),
                                );
                                for assumption in &entry.residual_assumptions {
                                    c.field("residual_assumption", assumption);
                                }
                            });
                        }
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
/// `vstd.vir` supplies the already-verified slice semantics. The full pinned
/// source-tree digest makes that model auditable, while `link_source_sha256`
/// and `link_rlib_sha256` bind the tiny `no_std` Rust metadata crate used only
/// for rustc name resolution and final linking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelVstdModelEvidence {
    pub vir_path: String,
    pub vir_sha256: String,
    pub source_root: String,
    pub source_file_count: u64,
    pub source_total_bytes: u64,
    pub source_sha256: String,
    pub atomic_source_path: String,
    pub atomic_source_sha256: String,
    pub full_rlib_path: String,
    pub full_rlib_sha256: String,
    pub link_source_name: String,
    pub link_source_sha256: String,
    pub link_build_args: Vec<String>,
    pub link_rlib_sha256: String,
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
    pub supported_target_features: Vec<String>,
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
        for feature in &self.supported_target_features {
            c.field("supported_target_feature", feature);
        }
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
    kernel_machine_vstd_rlib: Option<PathBuf>,
    _kernel_vstd_scratch: Option<ScratchTree>,
}

impl CollectedToolchain {
    fn dependency_path(&self, name: &str) -> Option<&Path> {
        self.dependency_paths.get(name).map(PathBuf::as_path)
    }

    fn activate_machine_vstd(&mut self) -> Result<(), ForgeError> {
        let path =
            self.kernel_machine_vstd_rlib
                .as_ref()
                .ok_or_else(|| ForgeError::VerusOutput {
                    detail: "machine-aware build has no pinned full vstd rlib".to_string(),
                })?;
        let model =
            self.evidence
                .kernel_vstd_model
                .as_ref()
                .ok_or_else(|| ForgeError::VerusOutput {
                    detail: "machine-aware build has no pinned kernel vstd model".to_string(),
                })?;
        if file_sha256(path)?.2 != model.full_rlib_sha256 {
            return Err(ForgeError::VerusOutput {
                detail: "pinned full vstd rlib drifted before machine-aware build".to_string(),
            });
        }
        let dependency = self
            .evidence
            .link_dependencies
            .iter_mut()
            .find(|dependency| dependency.name == "libvstd.rlib")
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: "machine-aware build has no vstd link dependency row".to_string(),
            })?;
        dependency.source_path = model.full_rlib_path.clone();
        dependency.sha256 = model.full_rlib_sha256.clone();
        self.dependency_paths
            .insert("libvstd.rlib".to_string(), path.clone());
        Ok(())
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
    #[serde(default)]
    pub primitive_crates: Vec<BoundPrimitiveCrateV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive_registry_sha256: Option<String>,
    #[serde(default)]
    pub reachable_primitive_count: u64,
    #[serde(default)]
    pub discharged_refinement_obligations: u64,
    #[serde(default)]
    pub residual_machine_assumptions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundPrimitiveObjectV1 {
    pub name: String,
    pub length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundPrimitiveCrateV1 {
    pub name: String,
    pub authored_source_sha256: String,
    pub crate_source_sha256: String,
    pub verus_result_sha256: String,
    pub vir_path: String,
    pub vir_length: u64,
    pub vir_sha256: String,
    pub rlib_path: String,
    pub rlib_length: u64,
    pub rlib_sha256: String,
    pub object_members: Vec<BoundPrimitiveObjectV1>,
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
                for primitive_crate in &composition.primitive_crates {
                    c.record("primitive_crate", |c| {
                        c.field("name", &primitive_crate.name);
                        c.field(
                            "authored_source_sha256",
                            &primitive_crate.authored_source_sha256,
                        );
                        c.field("crate_source_sha256", &primitive_crate.crate_source_sha256);
                        c.field("verus_result_sha256", &primitive_crate.verus_result_sha256);
                        c.field("vir_path", &primitive_crate.vir_path);
                        c.field("vir_length", &primitive_crate.vir_length.to_string());
                        c.field("vir_sha256", &primitive_crate.vir_sha256);
                        c.field("rlib_path", &primitive_crate.rlib_path);
                        c.field("rlib_length", &primitive_crate.rlib_length.to_string());
                        c.field("rlib_sha256", &primitive_crate.rlib_sha256);
                        for object in &primitive_crate.object_members {
                            c.record("object", |c| {
                                c.field("name", &object.name);
                                c.field("length", &object.length.to_string());
                                c.field("sha256", &object.sha256);
                            });
                        }
                    });
                }
                c.field(
                    "primitive_registry_sha256",
                    composition
                        .primitive_registry_sha256
                        .as_deref()
                        .unwrap_or(""),
                );
                c.field(
                    "reachable_primitive_count",
                    &composition.reachable_primitive_count.to_string(),
                );
                c.field(
                    "discharged_refinement_obligations",
                    &composition.discharged_refinement_obligations.to_string(),
                );
                c.field(
                    "residual_machine_assumptions",
                    &composition.residual_machine_assumptions.to_string(),
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
    }
}

fn reject(stage: &str, detail: impl Into<String>) -> VerifiedBuildOutcome {
    VerifiedBuildOutcome::Rejected {
        stage: stage.to_string(),
        detail: detail.into(),
    }
}

struct PreparedThermiteInput {
    raw_source: Vec<u8>,
    program: Program,
    package: Option<LoadedPackage>,
}

/// Freeze either a single source or a canonical package into the exact backend
/// projection while keeping package-local AST spans as the planning identity.
fn prepare_thermite_input(path: &Path) -> Result<PreparedThermiteInput, ForgeError> {
    let loaded = thermite_package::load(path)?;
    let source_text =
        std::str::from_utf8(&loaded.bytes).map_err(|error| ForgeError::VerusOutput {
            detail: format!("Thermite source is not UTF-8: {error}"),
        })?;
    let projected = thermite_syntax::parse(source_text);
    if !projected.is_clean() {
        return Err(ForgeError::Parse(projected.errors));
    }
    let program = match &loaded.package {
        Some(package) => {
            if normalized_program_sha256(&package.parsed.program)
                != normalized_program_sha256(&projected.program)
            {
                return Err(ForgeError::Package {
                    detail:
                        "independent module parsing disagrees with the canonical backend projection"
                            .to_string(),
                });
            }
            package.parsed.program.clone()
        }
        None => projected.program,
    };
    thermite_spec::validate(&program).map_err(ForgeError::Spec)?;
    thermite_lower::check_effects(&program).map_err(ForgeError::Effects)?;
    if let Some(package) = &loaded.package {
        validate_package_resolution(package, &program)?;
    }
    Ok(PreparedThermiteInput {
        raw_source: loaded.bytes,
        program,
        package: loaded.package,
    })
}

fn default_crate_name(path: &Path, package: Option<&LoadedPackage>) -> String {
    package.map_or_else(
        || sanitized_crate_name(path),
        |package| package.manifest.name.clone(),
    )
}

fn validate_package_resolution(
    package: &LoadedPackage,
    program: &Program,
) -> Result<(), ForgeError> {
    let item_modules: BTreeMap<&str, &str> = program
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let origin = package
                .parsed
                .origin(index)
                .expect("package origins are aligned with package items");
            (item.name(), origin.module.as_str())
        })
        .collect();
    let imports: BTreeMap<&str, BTreeSet<&str>> = package
        .manifest
        .modules
        .iter()
        .map(|module| {
            (
                module.name.as_str(),
                module.imports.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    let require_import = |from_item: &str, referenced_item: &str| -> Result<(), ForgeError> {
        let Some(from_module) = item_modules.get(from_item).copied() else {
            return Ok(());
        };
        let Some(to_module) = item_modules.get(referenced_item).copied() else {
            return Ok(());
        };
        if from_module != to_module && !imports[from_module].contains(to_module) {
            return Err(ForgeError::Package {
                detail: format!(
                    "module `{from_module}` uses `{referenced_item}` from module `{to_module}` without declaring that import"
                ),
            });
        }
        Ok(())
    };

    // Resolve every executable function, not only the requested export closure,
    // so an allowlisted package cannot hide an unresolved or undeclared
    // cross-module call in a sibling item.
    let roots: Vec<String> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) => Some(function.name.clone()),
            _ => None,
        })
        .collect();
    let closure =
        closure::verified_closure(program, &roots).map_err(|error| ForgeError::Package {
            detail: error.to_string(),
        })?;
    for (from, to) in &closure.edges {
        require_import(from, to)?;
    }

    let type_modules: BTreeMap<&str, &str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(_) | Item::Enum(_) => Some((item.name(), item_modules[item.name()])),
            _ => None,
        })
        .collect();
    for item in &program.items {
        let mut referenced = BTreeSet::new();
        collect_item_named_types(item, &mut referenced);
        for name in referenced {
            if type_modules.contains_key(name) {
                require_import(item.name(), name)?;
            }
        }
    }

    let capacity_modules: BTreeMap<&str, &str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(_) => Some((item.name(), item_modules[item.name()])),
            _ => None,
        })
        .collect();
    for item in &program.items {
        let mut referenced = BTreeSet::new();
        collect_item_capacity_refs(item, &mut referenced);
        for name in referenced {
            if capacity_modules.contains_key(name) {
                require_import(item.name(), name)?;
            }
        }
    }

    // `#[opaque]` is a package construction barrier: verified code in the
    // declaring module may build the state, while every other module must obtain
    // it through that module's functions. Resolve the complete item expression
    // tree, not only the requested export closure, so an unreachable sibling
    // cannot hide a forged opaque value in a receipt-bound package.
    let opaque_modules: BTreeMap<&str, &str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(structure) if structure.opaque => Some((
                structure.name.as_str(),
                item_modules[structure.name.as_str()],
            )),
            _ => None,
        })
        .collect();
    for (index, item) in program.items.iter().enumerate() {
        let from_module = package
            .parsed
            .origin(index)
            .expect("package origins are aligned with package items")
            .module
            .as_str();
        let mut constructed = BTreeSet::new();
        collect_item_struct_literals(item, &mut constructed);
        for name in constructed {
            let Some(defining_module) = opaque_modules.get(name).copied() else {
                continue;
            };
            if from_module != defining_module {
                return Err(ForgeError::Package {
                    detail: format!(
                        "module `{from_module}` constructs `#[opaque]` type `{name}` declared in module `{defining_module}`; opaque struct literals are permitted only in the defining module"
                    ),
                });
            }
        }
    }

    // Opaque state owns its representation, not only its constructors. Reject
    // direct field projection/mutation from every foreign package module. A
    // foreign module may carry the abstract type and call a verified observer or
    // transition, but it may not depend on the generated crate-visible fields.
    let record_fields: BTreeMap<String, BTreeMap<String, Type>> = program
        .items
        .iter()
        .filter_map(|item| {
            let Item::Struct(structure) = item else {
                return None;
            };
            Some((
                structure.name.clone(),
                structure
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect(),
            ))
        })
        .collect();
    let call_returns: BTreeMap<String, Type> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) => Some((function.name.clone(), function.ret.clone())),
            Item::SpecFn(function) => Some((function.name.clone(), function.ret.clone())),
            _ => None,
        })
        .collect();
    let opaque_field_owners: BTreeMap<String, BTreeSet<String>> = program
        .items
        .iter()
        .filter_map(|item| {
            let Item::Struct(structure) = item else {
                return None;
            };
            structure.opaque.then_some(structure)
        })
        .flat_map(|structure| {
            structure
                .fields
                .iter()
                .map(move |field| (field.name.clone(), structure.name.clone()))
        })
        .fold(BTreeMap::new(), |mut fields, (field, owner)| {
            fields.entry(field).or_default().insert(owner);
            fields
        });
    for (index, item) in program.items.iter().enumerate() {
        let from_module = package
            .parsed
            .origin(index)
            .expect("package origins are aligned with package items")
            .module
            .as_str();
        let mut accessed = BTreeSet::new();
        let mut unresolved = BTreeSet::new();
        collect_item_record_field_owners(
            item,
            &record_fields,
            &call_returns,
            &mut accessed,
            &mut unresolved,
        );
        for name in accessed {
            let Some(defining_module) = opaque_modules.get(name.as_str()).copied() else {
                continue;
            };
            if from_module != defining_module {
                return Err(ForgeError::Package {
                    detail: format!(
                        "module `{from_module}` accesses a field of `#[opaque]` type `{name}` declared in module `{defining_module}`; opaque representation reads and writes are permitted only in the defining module"
                    ),
                });
            }
        }
        for field in unresolved {
            let Some(possible_owners) = opaque_field_owners.get(&field) else {
                continue;
            };
            let foreign: Vec<_> = possible_owners
                .iter()
                .filter(|owner| {
                    opaque_modules
                        .get(owner.as_str())
                        .is_some_and(|module| *module != from_module)
                })
                .cloned()
                .collect();
            if !foreign.is_empty() {
                return Err(ForgeError::Package {
                    detail: format!(
                        "module `{from_module}` accesses field `{field}` through a receiver whose record type cannot be resolved before code generation; the field belongs to foreign `#[opaque]` type(s) {}, so the package fails closed rather than permitting an unverified representation access",
                        foreign.join(", ")
                    ),
                });
            }
        }
    }
    Ok(())
}

fn collect_item_record_field_owners(
    item: &Item,
    records: &BTreeMap<String, BTreeMap<String, Type>>,
    call_returns: &BTreeMap<String, Type>,
    owners: &mut BTreeSet<String>,
    unresolved: &mut BTreeSet<String>,
) {
    match item {
        Item::Fn(function) => {
            let mut env: BTreeMap<String, Type> = function
                .params
                .iter()
                .map(|param| (param.name.clone(), param.ty.clone()))
                .collect();
            env.insert("result".to_string(), function.ret.clone());
            collect_expr_record_field_owners(
                &function.contract.req.expr,
                &env,
                records,
                call_returns,
                owners,
                unresolved,
            );
            for clause in &function.contract.ens {
                collect_expr_record_field_owners(
                    &clause.expr,
                    &env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
            if let Some(dec) = &function.dec {
                collect_expr_record_field_owners(
                    &dec.expr,
                    &env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
            if let Some(body) = &function.body {
                collect_block_record_field_owners(
                    body,
                    &mut env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
        }
        Item::SpecFn(function) => {
            let mut env: BTreeMap<String, Type> = function
                .params
                .iter()
                .map(|param| (param.name.clone(), param.ty.clone()))
                .collect();
            collect_expr_record_field_owners(
                &function.dec.expr,
                &env,
                records,
                call_returns,
                owners,
                unresolved,
            );
            collect_block_record_field_owners(
                &function.body,
                &mut env,
                records,
                call_returns,
                owners,
                unresolved,
            );
        }
        Item::Struct(structure) => {
            let env: BTreeMap<String, Type> = structure
                .fields
                .iter()
                .map(|field| (field.name.clone(), field.ty.clone()))
                .collect();
            if let Some(inv) = &structure.inv {
                collect_expr_record_field_owners(
                    &inv.expr,
                    &env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
        }
        Item::Enum(_) | Item::Const(_) | Item::Forge(_) => {}
    }
}

fn collect_block_record_field_owners(
    block: &Block,
    env: &mut BTreeMap<String, Type>,
    records: &BTreeMap<String, BTreeMap<String, Type>>,
    call_returns: &BTreeMap<String, Type>,
    owners: &mut BTreeSet<String>,
    unresolved: &mut BTreeSet<String>,
) {
    for statement in &block.stmts {
        match statement {
            Stmt::Let { name, ty, init, .. } => {
                collect_expr_record_field_owners(
                    init,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
                if let Some(ty) = ty
                    .clone()
                    .or_else(|| record_value_type(init, env, records, call_returns))
                {
                    env.insert(name.clone(), ty);
                }
            }
            Stmt::Assign { target, value } => {
                collect_expr_record_field_owners(
                    target,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
                collect_expr_record_field_owners(
                    value,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
            Stmt::Return(Some(value)) | Stmt::Expr(value) => collect_expr_record_field_owners(
                value,
                env,
                records,
                call_returns,
                owners,
                unresolved,
            ),
            Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
            Stmt::If { cond, then, else_ } => {
                collect_expr_record_field_owners(
                    cond,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
                collect_block_record_field_owners(
                    then,
                    &mut env.clone(),
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
                if let Some(else_) = else_ {
                    collect_block_record_field_owners(
                        else_,
                        &mut env.clone(),
                        records,
                        call_returns,
                        owners,
                        unresolved,
                    );
                }
            }
            Stmt::Loop(node) => {
                if let thermite_syntax::LoopKind::While(cond) = &node.kind {
                    collect_expr_record_field_owners(
                        cond,
                        env,
                        records,
                        call_returns,
                        owners,
                        unresolved,
                    );
                }
                for invariant in &node.invs {
                    collect_expr_record_field_owners(
                        &invariant.expr,
                        env,
                        records,
                        call_returns,
                        owners,
                        unresolved,
                    );
                }
                collect_expr_record_field_owners(
                    &node.dec.expr,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
                collect_block_record_field_owners(
                    &node.body,
                    &mut env.clone(),
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
        }
    }
    if let Some(tail) = &block.tail {
        collect_expr_record_field_owners(tail, env, records, call_returns, owners, unresolved);
    }
}

fn collect_expr_record_field_owners(
    expr: &Expr,
    env: &BTreeMap<String, Type>,
    records: &BTreeMap<String, BTreeMap<String, Type>>,
    call_returns: &BTreeMap<String, Type>,
    owners: &mut BTreeSet<String>,
    unresolved: &mut BTreeSet<String>,
) {
    if let Expr::Field { receiver, name } = expr {
        match record_value_type(receiver, env, records, call_returns).and_then(record_type_name) {
            Some(owner) => {
                owners.insert(owner);
            }
            None => {
                unresolved.insert(name.clone());
            }
        }
    }
    match expr {
        Expr::Array(values) | Expr::Tuple(values) => {
            for value in values {
                collect_expr_record_field_owners(
                    value,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
        }
        Expr::ArrayRepeat { value, .. }
        | Expr::Unary { expr: value, .. }
        | Expr::Cast { expr: value, .. }
        | Expr::Ref { expr: value, .. }
        | Expr::Deref(value)
        | Expr::TupleProj {
            receiver: value, ..
        } => {
            collect_expr_record_field_owners(value, env, records, call_returns, owners, unresolved)
        }
        Expr::Call { callee, args } => {
            collect_expr_record_field_owners(
                callee,
                env,
                records,
                call_returns,
                owners,
                unresolved,
            );
            for arg in args {
                collect_expr_record_field_owners(
                    arg,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_record_field_owners(
                receiver,
                env,
                records,
                call_returns,
                owners,
                unresolved,
            );
            for arg in args {
                collect_expr_record_field_owners(
                    arg,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
        }
        Expr::Field { receiver, .. } => collect_expr_record_field_owners(
            receiver,
            env,
            records,
            call_returns,
            owners,
            unresolved,
        ),
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_record_field_owners(lhs, env, records, call_returns, owners, unresolved);
            collect_expr_record_field_owners(rhs, env, records, call_returns, owners, unresolved);
        }
        Expr::Index { base, index } => {
            collect_expr_record_field_owners(base, env, records, call_returns, owners, unresolved);
            match index {
                thermite_syntax::IndexArg::Single(index)
                | thermite_syntax::IndexArg::RangeTo(index)
                | thermite_syntax::IndexArg::RangeFrom(index) => collect_expr_record_field_owners(
                    index,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                ),
                thermite_syntax::IndexArg::Range(start, end) => {
                    collect_expr_record_field_owners(
                        start,
                        env,
                        records,
                        call_returns,
                        owners,
                        unresolved,
                    );
                    collect_expr_record_field_owners(
                        end,
                        env,
                        records,
                        call_returns,
                        owners,
                        unresolved,
                    );
                }
            }
        }
        Expr::Closure { body, .. } => {
            collect_expr_record_field_owners(body, env, records, call_returns, owners, unresolved)
        }
        Expr::Match { scrutinee, arms } => {
            collect_expr_record_field_owners(
                scrutinee,
                env,
                records,
                call_returns,
                owners,
                unresolved,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_record_field_owners(
                        guard,
                        env,
                        records,
                        call_returns,
                        owners,
                        unresolved,
                    );
                }
                collect_expr_record_field_owners(
                    &arm.body,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_expr_record_field_owners(cond, env, records, call_returns, owners, unresolved);
            collect_block_record_field_owners(
                then,
                &mut env.clone(),
                records,
                call_returns,
                owners,
                unresolved,
            );
            collect_block_record_field_owners(
                else_,
                &mut env.clone(),
                records,
                call_returns,
                owners,
                unresolved,
            );
        }
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                collect_expr_record_field_owners(
                    value,
                    env,
                    records,
                    call_returns,
                    owners,
                    unresolved,
                );
            }
        }
        Expr::Is { scrutinee, .. } => collect_expr_record_field_owners(
            scrutinee,
            env,
            records,
            call_returns,
            owners,
            unresolved,
        ),
        Expr::Quantifier { domain, body, .. } => {
            collect_expr_record_field_owners(
                domain,
                env,
                records,
                call_returns,
                owners,
                unresolved,
            );
            collect_expr_record_field_owners(body, env, records, call_returns, owners, unresolved);
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

fn record_type_name(ty: Type) -> Option<String> {
    match ty {
        Type::Named(name) => Some(name),
        Type::Ref { inner, .. } => record_type_name(*inner),
        _ => None,
    }
}

fn record_value_type(
    expr: &Expr,
    env: &BTreeMap<String, Type>,
    records: &BTreeMap<String, BTreeMap<String, Type>>,
    call_returns: &BTreeMap<String, Type>,
) -> Option<Type> {
    match expr {
        Expr::Path(path) if path.len() == 1 => env.get(&path[0]).cloned(),
        Expr::Ref { mutable, expr } => Some(Type::Ref {
            mutable: *mutable,
            inner: Box::new(record_value_type(expr, env, records, call_returns)?),
        }),
        Expr::Deref(expr) => match record_value_type(expr, env, records, call_returns)? {
            Type::Ref { inner, .. } | Type::Box(inner) => Some(*inner),
            _ => None,
        },
        Expr::Call { callee, args }
            if matches!(callee.as_ref(), Expr::Path(path)
                if path.len() == 1 && matches!(path[0].as_str(), "old" | "final")) =>
        {
            let [arg] = args.as_slice() else {
                return None;
            };
            match record_value_type(arg, env, records, call_returns)? {
                Type::Ref { inner, .. } => Some(*inner),
                other => Some(other),
            }
        }
        Expr::Call { callee, .. } => {
            let Expr::Path(path) = callee.as_ref() else {
                return None;
            };
            let name = path.last()?;
            call_returns.get(name).cloned()
        }
        Expr::Field { receiver, name } => {
            let receiver = record_value_type(receiver, env, records, call_returns)?;
            let owner = match receiver {
                Type::Named(owner) => owner,
                Type::Ref { inner, .. } => match *inner {
                    Type::Named(owner) => owner,
                    _ => return None,
                },
                _ => return None,
            };
            records.get(&owner)?.get(name).cloned()
        }
        Expr::StructLit { path, .. } => {
            let name = path.last()?;
            records
                .contains_key(name)
                .then(|| Type::Named(name.clone()))
        }
        Expr::Tuple(values) => values
            .iter()
            .map(|value| record_value_type(value, env, records, call_returns))
            .collect::<Option<Vec<_>>>()
            .map(Type::Tuple),
        Expr::TupleProj { receiver, index } => {
            let Type::Tuple(elements) = record_value_type(receiver, env, records, call_returns)?
            else {
                return None;
            };
            elements.get(*index).cloned()
        }
        Expr::Index {
            base,
            index: thermite_syntax::IndexArg::Single(_),
        } => {
            let base = match record_value_type(base, env, records, call_returns)? {
                Type::Ref { inner, .. } => *inner,
                other => other,
            };
            match base {
                Type::Array { elem, .. } | Type::Slice(elem) | Type::Vec(elem) => Some(*elem),
                _ => None,
            }
        }
        Expr::Cast { ty, .. } => Some(ty.clone()),
        Expr::If { then, else_, .. } => {
            let then_ty = block_value_type(then, env, records, call_returns)?;
            let else_ty = block_value_type(else_, env, records, call_returns)?;
            (then_ty == else_ty).then_some(then_ty)
        }
        Expr::Match { arms, .. } => {
            let mut types = arms
                .iter()
                .map(|arm| record_value_type(&arm.body, env, records, call_returns));
            let first = types.next()??;
            types
                .all(|candidate| candidate.as_ref() == Some(&first))
                .then_some(first)
        }
        _ => None,
    }
}

fn block_value_type(
    block: &Block,
    env: &BTreeMap<String, Type>,
    records: &BTreeMap<String, BTreeMap<String, Type>>,
    call_returns: &BTreeMap<String, Type>,
) -> Option<Type> {
    let mut env = env.clone();
    for statement in &block.stmts {
        if let Stmt::Let { name, ty, init, .. } = statement {
            let ty = ty
                .clone()
                .or_else(|| record_value_type(init, &env, records, call_returns));
            if let Some(ty) = ty {
                env.insert(name.clone(), ty);
            }
        }
    }
    record_value_type(block.tail.as_deref()?, &env, records, call_returns)
}

fn collect_item_named_types<'a>(item: &'a Item, names: &mut BTreeSet<&'a str>) {
    match item {
        Item::Const(_) => {}
        Item::Fn(function) => collect_signature_named_types(&function.params, &function.ret, names),
        Item::SpecFn(function) => {
            collect_signature_named_types(&function.params, &function.ret, names)
        }
        Item::Struct(structure) => {
            for field in &structure.fields {
                collect_named_types(&field.ty, names);
            }
        }
        Item::Enum(enumeration) => {
            for variant in &enumeration.variants {
                match &variant.shape {
                    thermite_syntax::VariantShape::Unit => {}
                    thermite_syntax::VariantShape::Tuple(types) => {
                        for ty in types {
                            collect_named_types(ty, names);
                        }
                    }
                    thermite_syntax::VariantShape::Struct(fields) => {
                        for field in fields {
                            collect_named_types(&field.ty, names);
                        }
                    }
                }
            }
        }
        Item::Forge(ForgeItem::PropFn(function)) => {
            collect_signature_named_types(&function.params, &function.ret, names)
        }
        Item::Forge(ForgeItem::Lemma(lemma)) => {
            for param in &lemma.params {
                collect_named_types(&param.ty, names);
            }
        }
        Item::Forge(ForgeItem::Proof(_) | ForgeItem::Witness(_)) => {}
    }
}

fn collect_signature_named_types<'a>(
    params: &'a [thermite_syntax::Param],
    ret: &'a Type,
    names: &mut BTreeSet<&'a str>,
) {
    for param in params {
        collect_named_types(&param.ty, names);
    }
    collect_named_types(ret, names);
}

fn collect_named_types<'a>(ty: &'a Type, names: &mut BTreeSet<&'a str>) {
    match ty {
        Type::Named(name) => {
            names.insert(name);
        }
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Generic { arg: inner, .. }
        | Type::Box(inner)
        | Type::Vec(inner)
        | Type::Option(inner) => collect_named_types(inner, names),
        Type::Array { elem, .. } => collect_named_types(elem, names),
        Type::Result(ok, err) | Type::Map(ok, err) => {
            collect_named_types(ok, names);
            collect_named_types(err, names);
        }
        Type::Tuple(types) => {
            for ty in types {
                collect_named_types(ty, names);
            }
        }
        Type::Prim(_) | Type::Unit | Type::String => {}
    }
}

fn collect_item_capacity_refs<'a>(item: &'a Item, names: &mut BTreeSet<&'a str>) {
    match item {
        Item::Const(_) => {}
        Item::Fn(function) => {
            collect_signature_capacity_refs(&function.params, &function.ret, names);
            collect_expr_capacity_refs(&function.contract.req.expr, names);
            for clause in &function.contract.ens {
                collect_expr_capacity_refs(&clause.expr, names);
            }
            if let Some(clause) = &function.dec {
                collect_expr_capacity_refs(&clause.expr, names);
            }
            if let Some(body) = &function.body {
                collect_block_capacity_refs(body, names);
            }
        }
        Item::SpecFn(function) => {
            collect_signature_capacity_refs(&function.params, &function.ret, names);
            collect_expr_capacity_refs(&function.dec.expr, names);
            collect_block_capacity_refs(&function.body, names);
        }
        Item::Struct(structure) => {
            for field in &structure.fields {
                collect_type_capacity_refs(&field.ty, names);
            }
            if let Some(inv) = &structure.inv {
                collect_expr_capacity_refs(&inv.expr, names);
            }
        }
        Item::Enum(enumeration) => {
            for variant in &enumeration.variants {
                match &variant.shape {
                    thermite_syntax::VariantShape::Unit => {}
                    thermite_syntax::VariantShape::Tuple(types) => {
                        for ty in types {
                            collect_type_capacity_refs(ty, names);
                        }
                    }
                    thermite_syntax::VariantShape::Struct(fields) => {
                        for field in fields {
                            collect_type_capacity_refs(&field.ty, names);
                        }
                    }
                }
            }
        }
        Item::Forge(ForgeItem::PropFn(function)) => {
            collect_signature_capacity_refs(&function.params, &function.ret, names);
            if let Some(dec) = &function.dec {
                collect_expr_capacity_refs(&dec.expr, names);
            }
            collect_block_capacity_refs(&function.body, names);
        }
        Item::Forge(ForgeItem::Lemma(lemma)) => {
            for param in &lemma.params {
                collect_type_capacity_refs(&param.ty, names);
            }
            collect_expr_capacity_refs(&lemma.req.expr, names);
            for clause in &lemma.ens {
                collect_expr_capacity_refs(&clause.expr, names);
            }
        }
        Item::Forge(ForgeItem::Proof(_) | ForgeItem::Witness(_)) => {}
    }
}

fn collect_signature_capacity_refs<'a>(
    params: &'a [thermite_syntax::Param],
    ret: &'a Type,
    names: &mut BTreeSet<&'a str>,
) {
    for param in params {
        collect_type_capacity_refs(&param.ty, names);
    }
    collect_type_capacity_refs(ret, names);
}

fn collect_array_len_ref<'a>(len: &'a thermite_syntax::ArrayLen, names: &mut BTreeSet<&'a str>) {
    if let thermite_syntax::ArrayLen::Const(name) = len {
        names.insert(name);
    }
}

fn collect_type_capacity_refs<'a>(ty: &'a Type, names: &mut BTreeSet<&'a str>) {
    match ty {
        Type::Array { elem, len } => {
            collect_array_len_ref(len, names);
            collect_type_capacity_refs(elem, names);
        }
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Generic { arg: inner, .. }
        | Type::Box(inner)
        | Type::Vec(inner)
        | Type::Option(inner) => collect_type_capacity_refs(inner, names),
        Type::Result(ok, err) | Type::Map(ok, err) => {
            collect_type_capacity_refs(ok, names);
            collect_type_capacity_refs(err, names);
        }
        Type::Tuple(types) => {
            for ty in types {
                collect_type_capacity_refs(ty, names);
            }
        }
        Type::Prim(_) | Type::Unit | Type::String | Type::Named(_) => {}
    }
}

fn collect_block_capacity_refs<'a>(block: &'a Block, names: &mut BTreeSet<&'a str>) {
    for statement in &block.stmts {
        collect_stmt_capacity_refs(statement, names);
    }
    if let Some(tail) = &block.tail {
        collect_expr_capacity_refs(tail, names);
    }
}

fn collect_stmt_capacity_refs<'a>(statement: &'a Stmt, names: &mut BTreeSet<&'a str>) {
    match statement {
        Stmt::Let { ty, init, .. } => {
            if let Some(ty) = ty {
                collect_type_capacity_refs(ty, names);
            }
            collect_expr_capacity_refs(init, names);
        }
        Stmt::Assign { target, value } => {
            collect_expr_capacity_refs(target, names);
            collect_expr_capacity_refs(value, names);
        }
        Stmt::Return(Some(value)) | Stmt::Expr(value) => {
            collect_expr_capacity_refs(value, names);
        }
        Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
        Stmt::If { cond, then, else_ } => {
            collect_expr_capacity_refs(cond, names);
            collect_block_capacity_refs(then, names);
            if let Some(else_) = else_ {
                collect_block_capacity_refs(else_, names);
            }
        }
        Stmt::Loop(node) => {
            for inv in &node.invs {
                collect_expr_capacity_refs(&inv.expr, names);
            }
            collect_expr_capacity_refs(&node.dec.expr, names);
            if let thermite_syntax::LoopKind::While(cond) = &node.kind {
                collect_expr_capacity_refs(cond, names);
            }
            collect_block_capacity_refs(&node.body, names);
        }
    }
}

fn collect_expr_capacity_refs<'a>(expr: &'a Expr, names: &mut BTreeSet<&'a str>) {
    match expr {
        Expr::Array(elements) | Expr::Tuple(elements) => {
            for element in elements {
                collect_expr_capacity_refs(element, names);
            }
        }
        Expr::ArrayRepeat { value, len } => {
            collect_array_len_ref(len, names);
            collect_expr_capacity_refs(value, names);
        }
        Expr::Call { callee, args } => {
            collect_expr_capacity_refs(callee, names);
            for arg in args {
                collect_expr_capacity_refs(arg, names);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_capacity_refs(receiver, names);
            for arg in args {
                collect_expr_capacity_refs(arg, names);
            }
        }
        Expr::Field { receiver, .. }
        | Expr::TupleProj { receiver, .. }
        | Expr::Closure { body: receiver, .. }
        | Expr::Ref { expr: receiver, .. }
        | Expr::Deref(receiver)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Is {
            scrutinee: receiver,
            ..
        } => collect_expr_capacity_refs(receiver, names),
        Expr::Match { scrutinee, arms } => {
            collect_expr_capacity_refs(scrutinee, names);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_capacity_refs(guard, names);
                }
                collect_expr_capacity_refs(&arm.body, names);
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_expr_capacity_refs(cond, names);
            collect_block_capacity_refs(then, names);
            collect_block_capacity_refs(else_, names);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_capacity_refs(lhs, names);
            collect_expr_capacity_refs(rhs, names);
        }
        Expr::Index { base, index } => {
            collect_expr_capacity_refs(base, names);
            match index {
                thermite_syntax::IndexArg::Single(index)
                | thermite_syntax::IndexArg::RangeTo(index)
                | thermite_syntax::IndexArg::RangeFrom(index) => {
                    collect_expr_capacity_refs(index, names)
                }
                thermite_syntax::IndexArg::Range(start, end) => {
                    collect_expr_capacity_refs(start, names);
                    collect_expr_capacity_refs(end, names);
                }
            }
        }
        Expr::Cast { expr, ty } => {
            collect_expr_capacity_refs(expr, names);
            collect_type_capacity_refs(ty, names);
        }
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                collect_expr_capacity_refs(value, names);
            }
        }
        Expr::Quantifier { domain, body, .. } => {
            collect_expr_capacity_refs(domain, names);
            collect_expr_capacity_refs(body, names);
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

fn collect_item_struct_literals<'a>(item: &'a Item, names: &mut BTreeSet<&'a str>) {
    match item {
        Item::Const(_) | Item::Enum(_) => {}
        Item::Fn(function) => {
            collect_expr_struct_literals(&function.contract.req.expr, names);
            for clause in &function.contract.ens {
                collect_expr_struct_literals(&clause.expr, names);
            }
            if let Some(dec) = &function.dec {
                collect_expr_struct_literals(&dec.expr, names);
            }
            if let Some(body) = &function.body {
                collect_block_struct_literals(body, names);
            }
        }
        Item::SpecFn(function) => {
            collect_expr_struct_literals(&function.dec.expr, names);
            collect_block_struct_literals(&function.body, names);
        }
        Item::Struct(structure) => {
            if let Some(inv) = &structure.inv {
                collect_expr_struct_literals(&inv.expr, names);
            }
        }
        Item::Forge(ForgeItem::PropFn(function)) => {
            if let Some(dec) = &function.dec {
                collect_expr_struct_literals(&dec.expr, names);
            }
            collect_block_struct_literals(&function.body, names);
        }
        Item::Forge(ForgeItem::Lemma(lemma)) => {
            collect_expr_struct_literals(&lemma.req.expr, names);
            for clause in &lemma.ens {
                collect_expr_struct_literals(&clause.expr, names);
            }
        }
        // Proof blocks are opaque tactic text at this AST layer and cannot
        // contain executable Thermite expressions. Witness inhabitants are
        // parsed expressions and therefore participate in the barrier.
        Item::Forge(ForgeItem::Proof(_)) => {}
        Item::Forge(ForgeItem::Witness(witness)) => {
            for inhabit in &witness.inhabits {
                for argument in &inhabit.args {
                    collect_expr_struct_literals(argument, names);
                }
            }
        }
    }
}

fn collect_block_struct_literals<'a>(block: &'a Block, names: &mut BTreeSet<&'a str>) {
    for statement in &block.stmts {
        match statement {
            Stmt::Let { init, .. } | Stmt::Expr(init) | Stmt::Return(Some(init)) => {
                collect_expr_struct_literals(init, names);
            }
            Stmt::Assign { target, value } => {
                collect_expr_struct_literals(target, names);
                collect_expr_struct_literals(value, names);
            }
            Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
            Stmt::If { cond, then, else_ } => {
                collect_expr_struct_literals(cond, names);
                collect_block_struct_literals(then, names);
                if let Some(else_) = else_ {
                    collect_block_struct_literals(else_, names);
                }
            }
            Stmt::Loop(node) => {
                for inv in &node.invs {
                    collect_expr_struct_literals(&inv.expr, names);
                }
                collect_expr_struct_literals(&node.dec.expr, names);
                if let thermite_syntax::LoopKind::While(cond) = &node.kind {
                    collect_expr_struct_literals(cond, names);
                }
                collect_block_struct_literals(&node.body, names);
            }
        }
    }
    if let Some(tail) = &block.tail {
        collect_expr_struct_literals(tail, names);
    }
}

fn collect_expr_struct_literals<'a>(expr: &'a Expr, names: &mut BTreeSet<&'a str>) {
    match expr {
        Expr::Array(elements) | Expr::Tuple(elements) => {
            for element in elements {
                collect_expr_struct_literals(element, names);
            }
        }
        Expr::ArrayRepeat { value, .. } => collect_expr_struct_literals(value, names),
        Expr::Call { callee, args } => {
            collect_expr_struct_literals(callee, names);
            for argument in args {
                collect_expr_struct_literals(argument, names);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_struct_literals(receiver, names);
            for argument in args {
                collect_expr_struct_literals(argument, names);
            }
        }
        Expr::Field { receiver, .. }
        | Expr::TupleProj { receiver, .. }
        | Expr::Closure { body: receiver, .. }
        | Expr::Ref { expr: receiver, .. }
        | Expr::Deref(receiver)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Is {
            scrutinee: receiver,
            ..
        } => collect_expr_struct_literals(receiver, names),
        Expr::Match { scrutinee, arms } => {
            collect_expr_struct_literals(scrutinee, names);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_struct_literals(guard, names);
                }
                collect_expr_struct_literals(&arm.body, names);
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_expr_struct_literals(cond, names);
            collect_block_struct_literals(then, names);
            collect_block_struct_literals(else_, names);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_struct_literals(lhs, names);
            collect_expr_struct_literals(rhs, names);
        }
        Expr::Index { base, index } => {
            collect_expr_struct_literals(base, names);
            match index {
                thermite_syntax::IndexArg::Single(index)
                | thermite_syntax::IndexArg::RangeTo(index)
                | thermite_syntax::IndexArg::RangeFrom(index) => {
                    collect_expr_struct_literals(index, names)
                }
                thermite_syntax::IndexArg::Range(start, end) => {
                    collect_expr_struct_literals(start, names);
                    collect_expr_struct_literals(end, names);
                }
            }
        }
        Expr::Cast { expr, .. } => collect_expr_struct_literals(expr, names),
        Expr::StructLit { path, fields } => {
            if let Some(name) = path.last() {
                names.insert(name);
            }
            for (_, value) in fields {
                collect_expr_struct_literals(value, names);
            }
        }
        Expr::Quantifier { domain, body, .. } => {
            collect_expr_struct_literals(domain, names);
            collect_expr_struct_literals(body, names);
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) | Expr::StrLit(_) => {}
    }
}

fn package_exports_are_roots(package: &LoadedPackage, exports: &[String]) -> Result<(), String> {
    let roots: BTreeSet<&str> = package.manifest.roots.iter().map(String::as_str).collect();
    let item_modules: BTreeMap<&str, &str> = package
        .parsed
        .program
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            (
                item.name(),
                package.parsed.origin(index).unwrap().module.as_str(),
            )
        })
        .collect();
    for export in exports {
        if let Some(module) = item_modules.get(export.as_str()) {
            if !roots.contains(module) {
                return Err(format!(
                    "export `{export}` is declared in non-root module `{module}`; add that module to package roots or export a root-module API"
                ));
            }
        }
    }
    Ok(())
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
    let prepared = prepare_thermite_input(path)?;
    let raw_source = &prepared.raw_source;
    let program = &prepared.program;

    let crate_name = match crate_name {
        Some(name) if valid_crate_name(name) => name.to_string(),
        Some(name) => {
            return Ok(reject(
                "plan",
                format!("invalid crate name `{name}`; expected [A-Za-z_][A-Za-z0-9_]*"),
            ))
        }
        None => default_crate_name(path, prepared.package.as_ref()),
    };
    if exports.is_empty() {
        return Ok(reject(
            "plan",
            "an L3 build requires at least one explicit export",
        ));
    }
    if let Some(package) = &prepared.package {
        if let Err(detail) = package_exports_are_roots(package, exports) {
            return Ok(reject("package-exports", detail));
        }
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

    let closure = match closure::verified_closure(program, exports) {
        Ok(closure) => closure,
        Err(error) => return Ok(reject("closure", error.to_string())),
    };
    if let Some(detail) = strict_source_checks(program, &closure, target) {
        return Ok(reject("closure", detail));
    }

    let collected_toolchain = collect_toolchain(target)?;
    let toolchain = &collected_toolchain.evidence;
    let planned_exports = match plan_exports(
        program,
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
    let subprogram = closure_program(program, &closure);
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
        VerifiedTarget::Kernel => L3LibraryTarget::Kernel,
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
        raw_source,
        program,
        package: prepared.package.as_ref(),
        selected_program: &subprogram,
        closure: &closure,
        exports: &planned_exports,
        crate_name: &crate_name,
        target,
        target_triple: &toolchain.target_triple,
        target_pointer_width: &toolchain.target_pointer_width,
        target_endian: &toolchain.target_endian,
        target_features: &[],
        verus_imports: &[],
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
    let frozen_input_path = frozen_input.path.join("input.th");
    write_bytes(&frozen_input_path, raw_source)?;

    let mut certificates = check::check_file(&frozen_input_path)?;
    inject_certificate_fault(&mut certificates);
    if let Some(detail) = reject_certificates(&certificates, &closure, program) {
        return Ok(reject("certificates", detail));
    }

    let mut tv =
        collect_translation_validation(&frozen_input_path, program, &closure, &planned_exports)?;
    inject_tv_fault(&mut tv);
    if let Some(detail) = reject_translation_validation(&tv, program, &closure, &planned_exports) {
        return Ok(reject("translation-validation", detail));
    }

    if test_fault("before-verus") {
        return Ok(reject("fault-injection", "injected failure before Verus"));
    }
    let compiled = compile_verus_source(CompileVerusInput {
        crate_name: &crate_name,
        source: &verus_source,
        target,
        verus_path: &toolchain.verus_path,
        environment: &toolchain.environment,
        codegen_toolchain_sha256: &toolchain.artifact_codegen.canonical_identity_sha256(),
        kernel_vstd_rlib: collected_toolchain.dependency_path("libvstd.rlib"),
        target_features: &[],
        imports: &[],
        export_vir: false,
        kernel_vstd_model: true,
    })?;
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
        raw_source,
        package: prepared.package.as_ref(),
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
#[derive(Clone, Copy)]
pub struct CompositionSourcePaths<'a> {
    pub shells: &'a [PathBuf],
    pub primitive_registry: Option<&'a Path>,
}

pub fn build_composition_file(
    path: &Path,
    link_exports: &[String],
    composition_exports: &[String],
    sources: CompositionSourcePaths<'_>,
    crate_name: Option<&str>,
    out: Option<&Path>,
    target: VerifiedTarget,
) -> Result<VerifiedBuildOutcome, ForgeError> {
    composition::build_file(
        path,
        link_exports,
        composition_exports,
        sources,
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
            Item::Const(_) => false,
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
                // Capacity declarations are closed compile-time inputs. Keep all
                // of them in the selected program so isolated lowering never
                // drops a named length used by a reachable declaration or body.
                Item::Const(_) => true,
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
) -> Option<String> {
    strict_source_checks_with_registered_boundaries(program, closure, target, &BTreeSet::new())
}

fn strict_source_checks_with_registered_boundaries(
    program: &Program,
    closure: &VerifiedClosure,
    target: VerifiedTarget,
    registered_boundaries: &BTreeSet<String>,
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
                if (f.boundary.is_some() || f.body.is_none())
                    && !registered_boundaries.contains(&f.name)
                {
                    return Some(format!(
                        "reachable path `{}` crosses #[boundary] function `{}`",
                        closure_path(closure, &f.name).join(" -> "),
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
                if matches!(target, VerifiedTarget::Kernel)
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
    let structural_structs = thermite_spec::structural_array_equality_structs(program);
    let mutable_record_structs = thermite_spec::structural_record_mutation_structs(program);
    for name in roots {
        let function = program.items.iter().find_map(|item| match item {
            Item::Fn(f) if &f.name == name => Some(f),
            _ => None,
        });
        let Some(function) = function else {
            return Err(format!("unknown executable export `{name}`"));
        };
        if !function.params.iter().all(|p| {
            supported_public_param_type(&p.ty, &structural_structs, &mutable_record_structs)
        }) || !supported_public_return_type(
            &function.ret,
            &structural_structs,
            &mutable_record_structs,
        ) {
            return Err(format!(
                "export `{name}` has a type outside the verified public Rust ABI \
                 (finite plain values and shared/exclusive borrows of primitives, \
                 slices, fixed arrays with finite plain elements, and direct \
                 finite non-sealed record roots are supported; sealed, recursive, \
                 enum, reference-bearing, heap-backed, and nested opaque records \
                 are rejected)"
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
        let layout = public_abi_layout(program, function)?;
        let ownership = function
            .params
            .iter()
            .map(|param| abi_ownership(&param.ty).to_string())
            .collect::<Vec<_>>();
        let postcondition_ids = function
            .contract
            .ens
            .iter()
            .enumerate()
            .map(|(index, _)| format!("{}.ens#{}", function.name, index + 1))
            .collect::<Vec<_>>();
        let abi_preimage = format!(
            "thermite-rust-abi-v1\0crate={crate_name}\0profile={}\0triple={target_triple}\0pointer_width={target_pointer_width}\0endian={target_endian}\0ownership={}\0layout={layout}\0{signature}",
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

fn supported_public_param_type(
    ty: &Type,
    structural_structs: &BTreeSet<String>,
    mutable_record_structs: &BTreeSet<String>,
) -> bool {
    match ty {
        Type::Prim(_) | Type::Unit | Type::Tuple(_) | Type::Array { .. } => {
            supported_public_value_type(ty, structural_structs)
        }
        Type::Named(name) => mutable_record_structs.contains(name),
        Type::Ref { inner, .. } => match inner.as_ref() {
            Type::Slice(elem) | Type::Array { elem, .. } => {
                supported_public_storage_element(elem, structural_structs)
            }
            Type::Prim(_) => true,
            Type::Named(name) => mutable_record_structs.contains(name),
            _ => false,
        },
        _ => false,
    }
}

fn supported_public_return_type(
    ty: &Type,
    structural_structs: &BTreeSet<String>,
    mutable_record_structs: &BTreeSet<String>,
) -> bool {
    match ty {
        Type::Named(name) => mutable_record_structs.contains(name),
        _ => supported_public_value_type(ty, structural_structs),
    }
}

fn supported_public_value_type(ty: &Type, structural_structs: &BTreeSet<String>) -> bool {
    match ty {
        Type::Prim(_) | Type::Unit => true,
        Type::Array { elem, .. } => supported_public_storage_element(elem, structural_structs),
        Type::Tuple(elements) => elements
            .iter()
            .all(|element| supported_public_value_type(element, structural_structs)),
        Type::Named(name) => structural_structs.contains(name),
        _ => false,
    }
}

fn supported_public_storage_element(ty: &Type, structural_structs: &BTreeSet<String>) -> bool {
    supported_public_value_type(ty, structural_structs)
}

fn abi_ownership(ty: &Type) -> &'static str {
    match ty {
        Type::Ref { mutable: true, .. } => "exclusive_borrow",
        Type::Ref { mutable: false, .. } => "shared_borrow",
        _ => "by_value",
    }
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
        Type::Array { elem, len } => format!(
            "[{};{}]",
            abi_type(elem),
            match len {
                thermite_syntax::ArrayLen::Literal { value, .. } => value.to_string(),
                thermite_syntax::ArrayLen::Const(name) => name.clone(),
            }
        ),
        Type::Ref { mutable, inner } => {
            let borrow = if *mutable { "&mut " } else { "&" };
            format!("{borrow}{}", abi_type(inner))
        }
        Type::Slice(elem) => format!("[{}]", abi_type(elem)),
        Type::Tuple(elements) => format!(
            "({})",
            elements.iter().map(abi_type).collect::<Vec<_>>().join(",")
        ),
        Type::Named(name) => name.clone(),
        other => format!("unsupported:{other:?}"),
    }
}

/// Canonical transitive layout preimage for one public Rust export. Display
/// types intentionally preserve authored capacity names for diagnostics, but an
/// ABI fingerprint must change when the bound value of such a name or any field
/// in a reachable plain record changes. The exact compiler and target are added
/// by `plan_exports`; this function binds the source-level layout graph.
fn public_abi_layout(
    program: &Program,
    function: &thermite_syntax::FnItem,
) -> Result<String, String> {
    let mut visiting = BTreeSet::new();
    let params = function
        .params
        .iter()
        .map(|param| {
            abi_layout_type(program, &param.ty, &mut visiting)
                .map(|layout| format!("{}:{layout}", param.name))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = abi_layout_type(program, &function.ret, &mut visiting)?;
    Ok(format!("params({})->{result}", params.join(",")))
}

fn abi_layout_type(
    program: &Program,
    ty: &Type,
    visiting: &mut BTreeSet<String>,
) -> Result<String, String> {
    match ty {
        Type::Prim(_) | Type::Unit => Ok(abi_type(ty)),
        Type::Array { elem, len } => {
            let length = match len {
                thermite_syntax::ArrayLen::Literal { value, .. } => *value,
                thermite_syntax::ArrayLen::Const(name) => program
                    .items
                    .iter()
                    .find_map(|item| match item {
                        Item::Const(value) if value.name == *name => Some(value.value),
                        _ => None,
                    })
                    .ok_or_else(|| format!("public ABI references unresolved capacity `{name}`"))?,
            };
            Ok(format!(
                "array[{length};{}]",
                abi_layout_type(program, elem, visiting)?
            ))
        }
        Type::Tuple(elements) => Ok(format!(
            "tuple{}({})",
            elements.len(),
            elements
                .iter()
                .map(|element| abi_layout_type(program, element, visiting))
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        )),
        Type::Named(name) => {
            if !visiting.insert(name.clone()) {
                return Err(format!(
                    "public ABI record layout is recursive through `{name}`"
                ));
            }
            let structure = program.items.iter().find_map(|item| match item {
                Item::Struct(structure) if structure.name == *name => Some(structure),
                _ => None,
            });
            let Some(structure) = structure else {
                visiting.remove(name);
                return Err(format!("public ABI names undeclared record `{name}`"));
            };
            let fields = structure
                .fields
                .iter()
                .map(|field| {
                    abi_layout_type(program, &field.ty, visiting)
                        .map(|layout| format!("{}:{layout}", field.name))
                })
                .collect::<Result<Vec<_>, _>>();
            visiting.remove(name);
            Ok(format!(
                "struct:{name}:sealed={}:opaque={}{{{}}}",
                structure.sealed,
                structure.opaque,
                fields?.join(",")
            ))
        }
        Type::Ref { mutable, inner } => Ok(format!(
            "ref:{}({})",
            if *mutable { "mut" } else { "shared" },
            abi_layout_type(program, inner, visiting)?
        )),
        Type::Slice(elem) => Ok(format!(
            "slice({})",
            abi_layout_type(program, elem, visiting)?
        )),
        other => Err(format!(
            "public ABI layout cannot encode unsupported type {other:?}"
        )),
    }
}

fn executable_precondition(expr: &Expr) -> bool {
    match expr {
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::Path(_) => true,
        Expr::Binary { lhs, rhs, .. } => {
            executable_precondition(lhs) && executable_precondition(rhs)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => executable_precondition(expr),
        Expr::MethodCall {
            receiver,
            name,
            args,
        } if name == "len" && args.is_empty() => executable_precondition(receiver),
        _ => false,
    }
}

struct PlanInput<'a> {
    raw_source: &'a [u8],
    program: &'a Program,
    package: Option<&'a LoadedPackage>,
    selected_program: &'a Program,
    closure: &'a VerifiedClosure,
    exports: &'a [PlannedExport],
    crate_name: &'a str,
    target: VerifiedTarget,
    target_triple: &'a str,
    target_pointer_width: &'a str,
    target_endian: &'a str,
    target_features: &'a [String],
    verus_imports: &'a [String],
    verus_source: &'a str,
}

fn make_plan(input: PlanInput<'_>) -> ArtifactPlanV1 {
    let PlanInput {
        raw_source,
        program,
        package,
        selected_program,
        closure,
        exports,
        crate_name,
        target,
        target_triple,
        target_pointer_width,
        target_endian,
        target_features,
        verus_imports,
        verus_source,
    } = input;
    let mut nodes = Vec::new();
    let mut dispositions = Vec::new();
    for (item_index, item) in program.items.iter().enumerate() {
        let (included, kind) = match item {
            Item::Const(c) => (
                selected_program.items.iter().any(
                    |candidate| matches!(candidate, Item::Const(other) if other.name == c.name),
                ),
                "const",
            ),
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
                source_module: package
                    .and_then(|package| package.parsed.origin(item_index))
                    .map(|origin| origin.module.clone()),
                source_path: package
                    .and_then(|package| package.parsed.origin(item_index))
                    .map(|origin| origin.path.clone()),
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
            source_module: None,
            source_path: None,
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
            source_module: None,
            source_path: None,
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
        expected_verus_args: expected_verus_args(
            crate_name,
            target,
            target_features,
            verus_imports,
            false,
            true,
        ),
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
        package: package.map(package_plan),
        composition: None,
    }
}

fn package_plan(package: &LoadedPackage) -> PackagePlanV1 {
    let mapped: BTreeMap<&str, &thermite_package::PackageSourceMapModuleV1> = package
        .source_map
        .modules
        .iter()
        .map(|module| (module.name.as_str(), module))
        .collect();
    PackagePlanV1 {
        schema: package.manifest.schema.clone(),
        name: package.manifest.name.clone(),
        manifest_sha256: sha256(&package.manifest_bytes),
        source_map_sha256: sha256(&package.source_map_bytes),
        roots: package.manifest.roots.clone(),
        modules: package
            .modules
            .iter()
            .map(|module| {
                let source_map = mapped[module.declaration.name.as_str()];
                PlannedPackageModuleV1 {
                    name: module.declaration.name.clone(),
                    path: module.declaration.path.clone(),
                    imports: module.declaration.imports.clone(),
                    length: module.bytes.len() as u64,
                    sha256: source_map.source_sha256.clone(),
                    projection_source_start: source_map.projection_source_start,
                }
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
        Item::Const(item) => PlannedNodeParts {
            source_start: Some(item.span.start as u64),
            source_end: Some(item.span.end() as u64),
            body_sha256: Some(sha256(format!("{}:{}", item.name, item.value).as_bytes())),
            contract_sha256: None,
            effects_sha256: None,
        },
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
) -> Option<String> {
    reject_certificates_with_registered_boundaries(certs, closure, program, &BTreeSet::new())
}

fn reject_certificates_with_registered_boundaries(
    certs: &[Certificate],
    closure: &VerifiedClosure,
    program: &Program,
    registered_boundaries: &BTreeSet<String>,
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
                Item::Const(item) => Some((item.span.start, item.span.end())),
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
        if registered_boundaries.contains(name) {
            if cert.level != Level::L1
                || !cert.boundary
                || cert.slag
                || cert.lowered_assurance
                || cert.reject.is_some()
                || cert
                    .obligations
                    .iter()
                    .any(|obligation| obligation.status == ObligationStatus::Failed)
            {
                return Some(format!(
                    "registered boundary `{name}` does not carry the exact declared L1 boundary certificate completed by composition"
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
        match &cert.assurance_scope {
            None | Some(AssuranceScope::EndToEnd) => {}
            Some(AssuranceScope::ToBoundary { via }) if registered_boundaries.contains(via) => {}
            Some(AssuranceScope::ToBoundary { .. }) => {
                return Some(format!(
                    "reachable node `{name}` crosses an unregistered boundary"
                ));
            }
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

fn assurance_aggregate_with_registered_boundaries(
    certificates: &[Certificate],
    closure: &VerifiedClosure,
    exports: &[PlannedExport],
    registered_boundaries: &BTreeSet<String>,
    machine_boundaries: &BTreeSet<String>,
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
        let achieved = if machine_boundaries.contains(name) {
            minimum = minimum.min(Level::L1);
            "L1-residual-machine-assumption".to_string()
        } else if registered_boundaries.contains(name) {
            minimum = minimum.min(Level::L3);
            "L3-direct-refinement".to_string()
        } else {
            minimum = minimum.min(certificate.level);
            level_name(certificate.level).to_string()
        };
        members.push(AssuranceMember {
            name: name.to_string(),
            kind: if machine_boundaries.contains(name) {
                "frozen_machine_boundary".to_string()
            } else if registered_boundaries.contains(name) {
                "frozen_primitive_boundary".to_string()
            } else if closure.functions.contains(name) {
                "executable".to_string()
            } else {
                "specification".to_string()
            },
            achieved,
        });
        if machine_boundaries.contains(name) {
            members.push(AssuranceMember {
                name: format!("{name}::checked_wrapper"),
                kind: "machine_refinement_wrapper".to_string(),
                achieved: "L3-relative-to-pinned-machine-model".to_string(),
            });
        }
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
    if machine_boundaries.is_empty() && minimum < Level::L3 {
        return Err(ForgeError::VerusOutput {
            detail: format!("verified artifact aggregate fell below L3 at {minimum_reachable}"),
        });
    }
    if !machine_boundaries.is_empty() && minimum != Level::L1 {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "machine-aware artifact must retain an L1 residual boundary, observed {minimum_reachable}"
            ),
        });
    }
    Ok(AssuranceAggregate {
        headline: if machine_boundaries.is_empty() {
            "L3".to_string()
        } else {
            "L1".to_string()
        },
        cap: if machine_boundaries.is_empty() {
            "L3".to_string()
        } else {
            "L1-machine-residual".to_string()
        },
        minimum_reachable,
        scope: if machine_boundaries.is_empty() {
            "end_to_end".to_string()
        } else {
            "to_machine_boundary".to_string()
        },
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
        let result = crate::exec_tv::exec_tv_export_guard(
            program,
            function,
            DEFAULT_SOLVER_SEED,
            DEFAULT_RLIMIT,
        );
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
                    Stmt::Let {
                        ty: Some(_), init, ..
                    } if !crate::exec_tv::expr_contains_body_control(init)
                        && !crate::exec_tv::is_direct_mutable_call(program, init) =>
                    {
                        let_index += 1;
                        expect_tv(
                            &mut expected,
                            "exec",
                            format!("{}.let#{let_index}", function.name),
                        );
                    }
                    Stmt::Let { .. } => {
                        // Body-TV owns control-flow values; still advance the
                        // source-order label counter used for later leaf lets.
                        let_index += 1;
                    }
                    Stmt::Return(Some(value))
                        if !crate::exec_tv::expr_contains_body_control(value) =>
                    {
                        expect_tv(&mut expected, "exec", format!("{}.return", function.name))
                    }
                    _ => {}
                }
            }
            if body
                .tail
                .as_deref()
                .is_some_and(|tail| !crate::exec_tv::expr_contains_body_control(tail))
            {
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
    let artifact_codegen = collect_codegen_rustc(&verus_version, &rustup)?;
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
    let mut kernel_machine_vstd_rlib = None;
    let verus_dir = verus.parent().ok_or_else(|| ForgeError::VerusOutput {
        detail: "the resolved Verus binary has no installation directory".to_string(),
    })?;
    if matches!(target, VerifiedTarget::Kernel) {
        let (scratch, dependency, model) = build_kernel_vstd_link(&verus, verus_dir, &environment)?;
        dependency_paths.insert(dependency.name.clone(), scratch.path.join("libvstd.rlib"));
        link_dependencies.push(dependency);
        kernel_vstd_model = Some(model);
        kernel_vstd_scratch = Some(scratch);
        kernel_machine_vstd_rlib = Some(verus_dir.join("libvstd.rlib"));
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
        kernel_machine_vstd_rlib,
        _kernel_vstd_scratch: kernel_vstd_scratch,
    })
}

fn kernel_vstd_link_build_args() -> Vec<String> {
    vec![
        KERNEL_VSTD_LINK_SOURCE_NAME.to_string(),
        "--is-vstd".to_string(),
        "--no-verify".to_string(),
        "--compile".to_string(),
        "--crate-type=rlib".to_string(),
        "--crate-name".to_string(),
        "vstd".to_string(),
        "--out-dir".to_string(),
        "<SCRATCH>".to_string(),
        "--remap-path-prefix=<SCRATCH>=.".to_string(),
    ]
}

fn build_kernel_vstd_link(
    verus: &Path,
    verus_dir: &Path,
    environment: &BTreeMap<String, String>,
) -> Result<(ScratchTree, ToolchainDependency, KernelVstdModelEvidence), ForgeError> {
    let vir = verus_dir.join("vstd.vir");
    let source_root = verus_dir.join("vstd");
    let atomic_source = source_root.join("atomic.rs");
    let full_rlib = verus_dir.join("libvstd.rlib");
    if !vir.is_file() || !source_root.is_dir() || !atomic_source.is_file() || !full_rlib.is_file() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "kernel slice model requires `{}` and `{}`",
                vir.display(),
                source_root.display()
            ),
        });
    }
    let (source_file_count, source_total_bytes, source_sha256) =
        directory_sha256_named(&source_root, "thermite.kernel-vstd-source-tree.v1")?;
    let scratch = ScratchTree::new_in_temp("kernel_vstd_link")?;
    let link_source = scratch.path.join(KERNEL_VSTD_LINK_SOURCE_NAME);
    write_bytes(&link_source, KERNEL_VSTD_LINK_SOURCE.as_bytes())?;

    let mut command = Command::new(verus);
    command
        .arg(KERNEL_VSTD_LINK_SOURCE_NAME)
        .args([
            "--is-vstd",
            "--no-verify",
            "--compile",
            "--crate-type=rlib",
            "--crate-name",
            "vstd",
            "--out-dir",
            ".",
        ])
        .arg(format!("--remap-path-prefix={}=.", scratch.path.display()))
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
    let dependency = ToolchainDependency {
        name: "libvstd.rlib".to_string(),
        source_path: "<forge-generated:kernel-vstd-link.rs>".to_string(),
        sha256: link_rlib_sha256.clone(),
    };
    let model = KernelVstdModelEvidence {
        vir_path: vir.display().to_string(),
        vir_sha256: file_sha256(&vir)?.2,
        source_root: source_root.display().to_string(),
        source_file_count,
        source_total_bytes,
        source_sha256,
        atomic_source_path: atomic_source.display().to_string(),
        atomic_source_sha256: file_sha256(&atomic_source)?.2,
        full_rlib_path: full_rlib.display().to_string(),
        full_rlib_sha256: file_sha256(&full_rlib)?.2,
        link_source_name: KERNEL_VSTD_LINK_SOURCE_NAME.to_string(),
        link_source_sha256: sha256(KERNEL_VSTD_LINK_SOURCE.as_bytes()),
        link_build_args: kernel_vstd_link_build_args(),
        link_rlib_sha256,
    };
    Ok((scratch, dependency, model))
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
    let target_triple = rustc_version_field(&rustc_version, "host: ")?;
    let rustc_release = rustc_version_field(&rustc_version, "release: ")?;
    let rustc_commit_hash = rustc_version_field(&rustc_version, "commit-hash: ")?;
    let llvm_version = rustc_version_field(&rustc_version, "LLVM version: ")?;
    let cfg = rustup_command_text(
        rustup,
        &rustup_toolchain,
        &["rustc", "--print", "cfg"],
        "Verus codegen rustc --print cfg",
    )?;
    let target_pointer_width = rustc_cfg_value(&cfg, "target_pointer_width")?;
    let target_endian = rustc_cfg_value(&cfg, "target_endian")?;
    let supported_target_features = parse_rustc_target_features(&rustup_command_text(
        rustup,
        &rustup_toolchain,
        &["rustc", "--print", "target-features"],
        "Verus codegen rustc --print target-features",
    )?)?;
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
    let target_libdir_text = rustup_command_text(
        rustup,
        &rustup_toolchain,
        &["rustc", "--print", "target-libdir"],
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
    let rustc_manifest = rustlib.join(format!("manifest-rustc-{target_triple}"));
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
        supported_target_features,
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

fn parse_rustc_target_features(output: &str) -> Result<Vec<String>, ForgeError> {
    let mut features = Vec::new();
    for line in output.lines() {
        let Some((name, _description)) = line.trim().split_once(" - ") else {
            continue;
        };
        let name = name.trim();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "codegen rustc --print target-features emitted invalid name `{name}`"
                ),
            });
        }
        features.push(name.to_string());
    }
    features.sort();
    if features.is_empty() || features.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ForgeError::VerusOutput {
            detail: "codegen rustc target-feature inventory is empty or contains duplicates"
                .to_string(),
        });
    }
    Ok(features)
}

fn require_supported_target_features(
    requested: &[String],
    supported: &[String],
) -> Result<(), String> {
    if let Some(feature) = requested
        .iter()
        .find(|feature| supported.binary_search(feature).is_err())
    {
        return Err(format!(
            "primitive registry target feature `{feature}` is not in the pinned codegen rustc target-feature inventory"
        ));
    }
    Ok(())
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
    if codegen.rustc_release != release
        || codegen.rustc_commit_hash != commit
        || codegen.target_triple != host
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
    if codegen.supported_target_features.is_empty()
        || codegen
            .supported_target_features
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || codegen.supported_target_features.iter().any(|feature| {
            feature.is_empty()
                || !feature
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
    {
        return Err("recorded target-feature inventory is not canonical".to_string());
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
    exported_vir: Option<Vec<u8>>,
    object_members: Vec<BoundPrimitiveObjectV1>,
    evidence: VerusEvidence,
}

struct VerusImportBytes<'a> {
    name: &'a str,
    vir: &'a [u8],
    rlib: &'a [u8],
}

struct CompileVerusInput<'a> {
    crate_name: &'a str,
    source: &'a str,
    target: VerifiedTarget,
    verus_path: &'a str,
    environment: &'a BTreeMap<String, String>,
    codegen_toolchain_sha256: &'a str,
    kernel_vstd_rlib: Option<&'a Path>,
    target_features: &'a [String],
    imports: &'a [VerusImportBytes<'a>],
    export_vir: bool,
    kernel_vstd_model: bool,
}

fn compile_verus_source(input: CompileVerusInput<'_>) -> Result<CompiledVerus, ForgeError> {
    let CompileVerusInput {
        crate_name,
        source,
        target,
        verus_path,
        environment,
        codegen_toolchain_sha256,
        kernel_vstd_rlib,
        target_features,
        imports,
        export_vir,
        kernel_vstd_model,
    } = input;
    let import_names: Vec<String> = imports
        .iter()
        .map(|import| import.name.to_string())
        .collect();
    let args = expected_verus_args(
        crate_name,
        target,
        target_features,
        &import_names,
        export_vir,
        kernel_vstd_model,
    );
    let scratch = ScratchTree::new_in_temp(&format!("verified_{crate_name}"))?;
    let source_name = format!("{crate_name}.rs");
    let source_path = scratch.path.join(&source_name);
    write_bytes(&source_path, source.as_bytes())?;
    let before = file_sha256(&source_path)?.2;
    if !imports.is_empty() {
        let deps = scratch.path.join("deps");
        fs::create_dir(&deps).map_err(|source| ForgeError::Io {
            path: deps.display().to_string(),
            source,
        })?;
        for import in imports {
            write_bytes(&deps.join(format!("{}.vir", import.name)), import.vir)?;
            write_bytes(&deps.join(format!("lib{}.rlib", import.name)), import.rlib)?;
        }
    }
    let mut command = Command::new(verus_path);
    for arg in &args[..args.len() - 2] {
        match arg.as_str() {
            "vstd=<KERNEL_VSTD_VIR>" => {
                let verus_dir =
                    Path::new(verus_path)
                        .parent()
                        .ok_or_else(|| ForgeError::VerusOutput {
                            detail: "the resolved Verus binary has no installation directory"
                                .to_string(),
                        })?;
                command.arg(format!("vstd={}", verus_dir.join("vstd.vir").display()));
            }
            "vstd=<KERNEL_VSTD_RLIB>" => {
                let rlib = kernel_vstd_rlib.ok_or_else(|| ForgeError::VerusOutput {
                    detail: "kernel verification has no generated no_std vstd link crate"
                        .to_string(),
                })?;
                command.arg(format!("vstd={}", rlib.display()));
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
    };
    if !success {
        return Ok(CompiledVerus {
            artifact: Vec::new(),
            artifact_name: format!("lib{crate_name}.rlib"),
            exported_vir: None,
            object_members: Vec::new(),
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
    let exported_vir = if export_vir {
        let path = scratch.path.join(format!("{crate_name}.vir"));
        Some(fs::read(&path).map_err(|source| ForgeError::Io {
            path: path.display().to_string(),
            source,
        })?)
    } else {
        None
    };
    let object_members = archive_object_members(&artifact)?;
    Ok(CompiledVerus {
        artifact,
        artifact_name,
        exported_vir,
        object_members,
        evidence,
    })
}

fn expected_verus_args(
    crate_name: &str,
    target: VerifiedTarget,
    target_features: &[String],
    imports: &[String],
    export_vir: bool,
    kernel_vstd_model: bool,
) -> Vec<String> {
    let mut args = vec!["--output-json".to_string(), "--profile".to_string()];
    if export_vir {
        args.extend(["--export".to_string(), format!("{crate_name}.vir")]);
    }
    if matches!(target, VerifiedTarget::Kernel) {
        args.push("--no-vstd".to_string());
        if kernel_vstd_model {
            args.extend([
                "--import".to_string(),
                "vstd=<KERNEL_VSTD_VIR>".to_string(),
                "--extern".to_string(),
                "vstd=<KERNEL_VSTD_RLIB>".to_string(),
            ]);
        }
    }
    for import in imports {
        args.extend([
            "--import".to_string(),
            format!("{import}=deps/{import}.vir"),
            "--extern".to_string(),
            format!("{import}=deps/lib{import}.rlib"),
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
    ]);
    if !target_features.is_empty() {
        args.extend([
            "-C".to_string(),
            format!(
                "target-feature={}",
                target_features
                    .iter()
                    .map(|feature| format!("+{feature}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ]);
    }
    args.extend([
        "--remap-path-prefix=<SCRATCH>=.".to_string(),
        format!("{crate_name}.rs"),
    ]);
    args
}

fn archive_object_members(bytes: &[u8]) -> Result<Vec<BoundPrimitiveObjectV1>, ForgeError> {
    const MAGIC: &[u8] = b"!<arch>\n";
    if !bytes.starts_with(MAGIC) {
        return Err(ForgeError::VerusOutput {
            detail: "Verus rlib is not a canonical ar archive".to_string(),
        });
    }
    struct RawMember<'a> {
        name: String,
        data: &'a [u8],
    }
    let mut raw_members = Vec::new();
    let mut string_table: Option<&[u8]> = None;
    let mut offset = MAGIC.len();
    while offset < bytes.len() {
        if bytes.len() - offset < 60 {
            return Err(ForgeError::VerusOutput {
                detail: "Verus rlib has a truncated ar member header".to_string(),
            });
        }
        let header = &bytes[offset..offset + 60];
        if &header[58..60] != b"`\n" {
            return Err(ForgeError::VerusOutput {
                detail: "Verus rlib has an invalid ar member header".to_string(),
            });
        }
        let raw_name = std::str::from_utf8(&header[..16])
            .map_err(|_| ForgeError::VerusOutput {
                detail: "Verus rlib has a non-UTF-8 ar member name".to_string(),
            })?
            .trim()
            .to_string();
        let size = std::str::from_utf8(&header[48..58])
            .ok()
            .map(str::trim)
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: "Verus rlib has an invalid ar member size".to_string(),
            })?;
        let data_start = offset + 60;
        let data_end = data_start
            .checked_add(size)
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: "Verus rlib ar member size overflow".to_string(),
            })?;
        if data_end > bytes.len() {
            return Err(ForgeError::VerusOutput {
                detail: "Verus rlib has a truncated ar member".to_string(),
            });
        }
        let data = &bytes[data_start..data_end];
        if raw_name == "//" {
            string_table = Some(data);
        } else {
            raw_members.push(RawMember {
                name: raw_name,
                data,
            });
        }
        offset = data_end + (size & 1);
    }
    if offset != bytes.len() {
        return Err(ForgeError::VerusOutput {
            detail: "Verus rlib has invalid ar alignment padding".to_string(),
        });
    }

    let mut rows = Vec::new();
    let mut names = BTreeSet::new();
    for member in raw_members {
        let (name, data) =
            if let Some(length) = member.name.strip_prefix("#1/") {
                let length = length
                    .parse::<usize>()
                    .map_err(|_| ForgeError::VerusOutput {
                        detail: "Verus rlib has an invalid BSD extended member name".to_string(),
                    })?;
                if length > member.data.len() {
                    return Err(ForgeError::VerusOutput {
                        detail: "Verus rlib has a truncated BSD extended member name".to_string(),
                    });
                }
                let name = std::str::from_utf8(&member.data[..length])
                    .map_err(|_| ForgeError::VerusOutput {
                        detail: "Verus rlib has a non-UTF-8 BSD extended member name".to_string(),
                    })?
                    .trim_end_matches('\0')
                    .to_string();
                (name, &member.data[length..])
            } else if let Some(table_offset) = member.name.strip_prefix('/').filter(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
            }) {
                let table_offset =
                    table_offset
                        .parse::<usize>()
                        .map_err(|_| ForgeError::VerusOutput {
                            detail: "Verus rlib has an invalid GNU name-table offset".to_string(),
                        })?;
                let table = string_table.ok_or_else(|| ForgeError::VerusOutput {
                    detail: "Verus rlib references a missing GNU name table".to_string(),
                })?;
                let tail = table
                    .get(table_offset..)
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "Verus rlib GNU name-table offset is out of bounds".to_string(),
                    })?;
                let end = tail
                    .windows(2)
                    .position(|window| window == b"/\n")
                    .or_else(|| tail.iter().position(|byte| *byte == 0))
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "Verus rlib has an unterminated GNU member name".to_string(),
                    })?;
                let name = std::str::from_utf8(&tail[..end])
                    .map_err(|_| ForgeError::VerusOutput {
                        detail: "Verus rlib has a non-UTF-8 GNU member name".to_string(),
                    })?
                    .to_string();
                (name, member.data)
            } else {
                (member.name.trim_end_matches('/').to_string(), member.data)
            };
        if !name.ends_with(".o") {
            continue;
        }
        if !names.insert(name.clone()) {
            return Err(ForgeError::VerusOutput {
                detail: format!("Verus rlib has duplicate object member `{name}`"),
            });
        }
        rows.push(BoundPrimitiveObjectV1 {
            name,
            length: data.len() as u64,
            sha256: sha256(data),
        });
    }
    rows.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(rows)
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
    package: Option<&'a LoadedPackage>,
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
    primitive_registry: Option<&'a primitive_registry::PrimitiveRegistrySource>,
    primitive_crates: &'a [PrimitiveCrateSource],
    compiled_primitive_crates: &'a [CompiledVerus],
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
        thermite_package::write_evidence(&stage.path, package)?;
    }
    write_bytes(&evidence.join("artifact-plan.v1"), plan_json.as_bytes())?;
    write_bytes(&evidence.join("source.verus.rs"), verus_source.as_bytes())?;
    let mut bound_primitive_crates = Vec::new();
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
        if composition.primitive_crates.len() != composition.compiled_primitive_crates.len() {
            return Err(ForgeError::VerusOutput {
                detail: "separate primitive source/codegen cardinality mismatch".to_string(),
            });
        }
        for (source, compiled_primitive) in composition
            .primitive_crates
            .iter()
            .zip(composition.compiled_primitive_crates)
        {
            let primitive_dir = stage.path.join(
                Path::new(&source.plan.authored_source_path)
                    .parent()
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "separate primitive authored source has no parent".to_string(),
                    })?,
            );
            fs::create_dir_all(&primitive_dir).map_err(|source_error| ForgeError::Io {
                path: primitive_dir.display().to_string(),
                source: source_error,
            })?;
            write_bytes(
                &stage.path.join(&source.plan.authored_source_path),
                &source.authored_bytes,
            )?;
            write_bytes(
                &stage.path.join(&source.plan.crate_source_path),
                source.crate_source.as_bytes(),
            )?;
            let verus_result_path = format!(
                "{}/verus-result.json",
                Path::new(&source.plan.authored_source_path)
                    .parent()
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            let verus_result = pretty_json(
                &compiled_primitive.evidence,
                "separate primitive Verus evidence",
            )?;
            write_bytes(
                &stage.path.join(&verus_result_path),
                verus_result.as_bytes(),
            )?;
            let vir = compiled_primitive.exported_vir.as_ref().ok_or_else(|| {
                ForgeError::VerusOutput {
                    detail: format!(
                        "separate primitive crate `{}` has no exported Verus interface",
                        source.plan.name
                    ),
                }
            })?;
            let vir_path = format!(
                "{}/interface.vir",
                Path::new(&source.plan.authored_source_path)
                    .parent()
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            write_bytes(&stage.path.join(&vir_path), vir)?;
            let rlib_path = format!("artifact/deps/lib{}.rlib", source.plan.name);
            if compiled_primitive.artifact_name != format!("lib{}.rlib", source.plan.name) {
                return Err(ForgeError::VerusOutput {
                    detail: format!(
                        "separate primitive crate `{}` emitted unexpected artifact `{}`",
                        source.plan.name, compiled_primitive.artifact_name
                    ),
                });
            }
            write_bytes(&stage.path.join(&rlib_path), &compiled_primitive.artifact)?;
            bound_primitive_crates.push(BoundPrimitiveCrateV1 {
                name: source.plan.name.clone(),
                authored_source_sha256: source.plan.authored_source_sha256.clone(),
                crate_source_sha256: source.plan.crate_source_sha256.clone(),
                verus_result_sha256: sha256(verus_result.as_bytes()),
                vir_path,
                vir_length: vir.len() as u64,
                vir_sha256: sha256(vir),
                rlib_path,
                rlib_length: compiled_primitive.artifact.len() as u64,
                rlib_sha256: sha256(&compiled_primitive.artifact),
                object_members: compiled_primitive.object_members.clone(),
            });
        }
        if composition
            .primitive_crates
            .iter()
            .any(|source| source.plan.proof_basis == "pinned_vstd_machine_model")
        {
            let model =
                toolchain
                    .kernel_vstd_model
                    .as_ref()
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "machine primitive composition has no pinned kernel vstd model"
                            .to_string(),
                    })?;
            let atomic_source =
                fs::read(&model.atomic_source_path).map_err(|source| ForgeError::Io {
                    path: model.atomic_source_path.clone(),
                    source,
                })?;
            if sha256(&atomic_source) != model.atomic_source_sha256 {
                return Err(ForgeError::VerusOutput {
                    detail: "pinned vstd atomic source or full codegen rlib changed during machine composition"
                        .to_string(),
                });
            }
            fs::create_dir_all(stage.path.join("evidence/machine-models")).map_err(|source| {
                ForgeError::Io {
                    path: stage
                        .path
                        .join("evidence/machine-models")
                        .display()
                        .to_string(),
                    source,
                }
            })?;
            write_bytes(
                &stage.path.join(MACHINE_ATOMIC_MODEL_SOURCE_PATH),
                &atomic_source,
            )?;
        }
        if let Some(registry) = composition.primitive_registry {
            write_bytes(&stage.path.join(&registry.plan.path), &registry.bytes)?;
        }
    }
    write_bytes(&evidence.join("certificates.json"), cert_json.as_bytes())?;
    write_bytes(
        &evidence.join("translation-validation.json"),
        tv_json.as_bytes(),
    )?;
    write_bytes(&evidence.join("verus-result.json"), verus_json.as_bytes())?;
    write_bytes(&evidence.join("toolchain.json"), toolchain_json.as_bytes())?;
    if toolchain.kernel_vstd_model.is_some() {
        write_bytes(
            &evidence.join(KERNEL_VSTD_LINK_SOURCE_NAME),
            KERNEL_VSTD_LINK_SOURCE.as_bytes(),
        )?;
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
    let staged_closure = VerifiedClosure {
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
    };
    let registered_boundaries: BTreeSet<String> = plan
        .composition
        .as_ref()
        .and_then(|composition| composition.primitive_registry.as_ref())
        .map(|registry| {
            registry
                .entries
                .iter()
                .filter(|entry| entry.reachable)
                .map(|entry| entry.thermite_name.clone())
                .collect()
        })
        .unwrap_or_default();
    let machine_boundaries: BTreeSet<String> = plan
        .composition
        .as_ref()
        .and_then(|composition| composition.primitive_registry.as_ref())
        .map(|registry| {
            registry
                .entries
                .iter()
                .filter(|entry| entry.reachable && entry.machine_family.is_some())
                .map(|entry| entry.thermite_name.clone())
                .collect()
        })
        .unwrap_or_default();
    let assurance_aggregate = assurance_aggregate_with_registered_boundaries(
        certificates,
        &staged_closure,
        &plan.exports,
        &registered_boundaries,
        &machine_boundaries,
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
                primitive_crates: bound_primitive_crates.clone(),
                primitive_registry_sha256: composition
                    .primitive_registry
                    .as_ref()
                    .map(|registry| registry.sha256.clone()),
                reachable_primitive_count: composition
                    .primitive_registry
                    .as_ref()
                    .map(|registry| {
                        registry
                            .entries
                            .iter()
                            .filter(|entry| entry.reachable)
                            .count() as u64
                    })
                    .unwrap_or(0),
                discharged_refinement_obligations: composition
                    .primitive_registry
                    .as_ref()
                    .map(|registry| {
                        registry
                            .entries
                            .iter()
                            .filter(|entry| entry.reachable)
                            .map(|entry| entry.proof_obligations.len() as u64)
                            .sum()
                    })
                    .unwrap_or(0),
                residual_machine_assumptions: composition
                    .primitive_registry
                    .as_ref()
                    .map(|registry| {
                        registry
                            .entries
                            .iter()
                            .filter(|entry| entry.reachable)
                            .map(|entry| entry.residual_assumptions.len() as u64)
                            .sum()
                    })
                    .unwrap_or(0),
            });
    let binding = ReceiptBindingV1 {
        schema: receipt_schema.to_string(),
        assurance: assurance_aggregate.headline.clone(),
        scope: assurance_aggregate.scope.clone(),
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
    let mandatory_policy = if composition_receipt
        && receipt.binding.scope == "to_machine_boundary"
        && receipt.binding.assurance == "L1"
    {
        MACHINE_COMPOSITION_STRICT_GATES
    } else if composition_receipt {
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
                || binding.primitive_registry_sha256
                    != composition
                        .primitive_registry
                        .as_ref()
                        .map(|registry| registry.sha256.clone())
                || binding.reachable_primitive_count
                    != composition
                        .primitive_registry
                        .as_ref()
                        .map(|registry| {
                            registry
                                .entries
                                .iter()
                                .filter(|entry| entry.reachable)
                                .count() as u64
                        })
                        .unwrap_or(0)
                || binding.discharged_refinement_obligations
                    != composition
                        .primitive_registry
                        .as_ref()
                        .map(|registry| {
                            registry
                                .entries
                                .iter()
                                .filter(|entry| entry.reachable)
                                .map(|entry| entry.proof_obligations.len() as u64)
                                .sum()
                        })
                        .unwrap_or(0)
                || binding.residual_machine_assumptions
                    != composition
                        .primitive_registry
                        .as_ref()
                        .map(|registry| {
                            registry
                                .entries
                                .iter()
                                .filter(|entry| entry.reachable)
                                .map(|entry| entry.residual_assumptions.len() as u64)
                                .sum()
                        })
                        .unwrap_or(0)
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
    let has_machine_boundaries = plan
        .composition
        .as_ref()
        .and_then(|composition| composition.primitive_registry.as_ref())
        .is_some_and(|registry| {
            registry
                .entries
                .iter()
                .any(|entry| entry.reachable && entry.machine_family.is_some())
        });
    let expected_assurance = if has_machine_boundaries { "L1" } else { "L3" };
    let expected_scope = if has_machine_boundaries {
        "to_machine_boundary"
    } else {
        "end_to_end"
    };
    if receipt.binding.assurance != expected_assurance
        || receipt.binding.scope != expected_scope
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
    let source_text =
        std::str::from_utf8(&raw_source).map_err(|error| ForgeError::VerusOutput {
            detail: format!("bound Thermite input is not UTF-8: {error}"),
        })?;
    let projected = thermite_syntax::parse(source_text);
    if !projected.is_clean() {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "bound Thermite input no longer parses cleanly: {:?}",
                projected.errors
            ),
        });
    }
    let package = thermite_package::load_evidence(bundle, &raw_source)?;
    match (&plan.package, &package) {
        (None, None) => {}
        (Some(expected), Some(package)) if *expected == package_plan(package) => {}
        (Some(_), Some(_)) => {
            return Err(ForgeError::VerusOutput {
                detail: "bound package manifest, module closure, or source map disagrees with ArtifactPlanV1"
                    .to_string(),
            })
        }
        _ => {
            return Err(ForgeError::VerusOutput {
                detail: "ArtifactPlanV1 package presence disagrees with bound package evidence"
                    .to_string(),
            })
        }
    }
    let program = package.as_ref().map_or_else(
        || projected.program.clone(),
        |package| package.parsed.program.clone(),
    );
    if normalized_program_sha256(&program) != normalized_program_sha256(&projected.program) {
        return Err(ForgeError::VerusOutput {
            detail: "bound package modules disagree with their canonical backend projection"
                .to_string(),
        });
    }
    thermite_spec::validate(&program).map_err(|errors| ForgeError::VerusOutput {
        detail: format!("bound Thermite input fails spec validation: {errors:?}"),
    })?;
    thermite_lower::check_effects(&program).map_err(|errors| ForgeError::VerusOutput {
        detail: format!("bound Thermite input fails effect validation: {errors:?}"),
    })?;
    if let Some(package) = &package {
        validate_package_resolution(package, &program).map_err(|error| {
            ForgeError::VerusOutput {
                detail: format!("bound package fails module resolution: {error}"),
            }
        })?;
    }
    if normalized_program_sha256(&program) != plan.parsed_program_sha256 {
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
    if let Some(package) = &package {
        package_exports_are_roots(package, &roots).map_err(|detail| ForgeError::VerusOutput {
            detail: format!("bound package export plan is invalid: {detail}"),
        })?;
    }
    let closure =
        closure::verified_closure(&program, &roots).map_err(|error| ForgeError::VerusOutput {
            detail: format!("bound closure is incomplete: {error}"),
        })?;
    let planned_registered_boundaries: BTreeSet<String> = plan
        .composition
        .as_ref()
        .and_then(|composition| composition.primitive_registry.as_ref())
        .map(|registry| {
            registry
                .entries
                .iter()
                .filter(|entry| entry.reachable)
                .map(|entry| entry.thermite_name.clone())
                .collect()
        })
        .unwrap_or_default();
    if let Some(detail) = strict_source_checks_with_registered_boundaries(
        &program,
        &closure,
        plan.target,
        &planned_registered_boundaries,
    ) {
        return Err(ForgeError::VerusOutput {
            detail: format!("bound closure violates strict policy: {detail}"),
        });
    }
    let exports = plan_exports(
        &program,
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
    let subprogram = closure_program(&program, &closure);
    let (independently_emitted, reconstructed_plan) = if composition_receipt {
        let (reconstructed, lowered, combined, reconstructed_closure, reconstructed_exports) =
            composition::reconstruct_plan(&program, package.as_ref(), &raw_source, &plan, bundle)?;
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
            VerifiedTarget::Kernel => L3LibraryTarget::Kernel,
        };
        let emitted = thermite_lower::lower_l3_library(&subprogram, &lower_exports, lower_target)
            .map_err(ForgeError::Lower)?;
        let reconstructed = make_plan(PlanInput {
            raw_source: &raw_source,
            program: &program,
            package: package.as_ref(),
            selected_program: &subprogram,
            closure: &closure,
            exports: &exports,
            crate_name: &plan.crate_name,
            target: plan.target,
            target_triple: &plan.target_triple,
            target_pointer_width: &plan.target_pointer_width,
            target_endian: &plan.target_endian,
            target_features: &[],
            verus_imports: &[],
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
    let registered_boundaries: BTreeSet<String> = plan
        .composition
        .as_ref()
        .and_then(|composition| composition.primitive_registry.as_ref())
        .map(|registry| {
            registry
                .entries
                .iter()
                .filter(|entry| entry.reachable)
                .map(|entry| entry.thermite_name.clone())
                .collect()
        })
        .unwrap_or_default();
    let machine_boundaries: BTreeSet<String> = plan
        .composition
        .as_ref()
        .and_then(|composition| composition.primitive_registry.as_ref())
        .map(|registry| {
            registry
                .entries
                .iter()
                .filter(|entry| entry.reachable && entry.machine_family.is_some())
                .map(|entry| entry.thermite_name.clone())
                .collect()
        })
        .unwrap_or_default();
    if let Some(detail) = reject_certificates_with_registered_boundaries(
        &certificates,
        &closure,
        &program,
        &registered_boundaries,
    ) {
        return Err(ForgeError::VerusOutput {
            detail: format!("bound certificate set fails strict L3 policy: {detail}"),
        });
    }
    let reconstructed_assurance = assurance_aggregate_with_registered_boundaries(
        &certificates,
        &closure,
        &exports,
        &registered_boundaries,
        &machine_boundaries,
    )?;
    if receipt.binding.assurance_aggregate != reconstructed_assurance
        || reconstructed_assurance.headline != expected_assurance
        || reconstructed_assurance.scope != expected_scope
        || if has_machine_boundaries {
            reconstructed_assurance.cap != "L1-machine-residual"
                || reconstructed_assurance.minimum_reachable != "L1"
        } else {
            reconstructed_assurance.cap != "L3"
                || !matches!(
                    reconstructed_assurance.minimum_reachable.as_str(),
                    "L3" | "L4"
                )
        }
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
    if let Some(detail) = reject_translation_validation(&tv, &program, &closure, &exports) {
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
    let target_features = plan
        .composition
        .as_ref()
        .and_then(|composition| composition.primitive_registry.as_ref())
        .map(|registry| registry.target.target_features.as_slice())
        .unwrap_or(&[]);
    if !verus.success
        || verus.errors != Some(0)
        || verus.args != plan.expected_verus_args
        || plan.expected_verus_args
            != expected_verus_args(
                &plan.crate_name,
                plan.target,
                target_features,
                &plan
                    .composition
                    .as_ref()
                    .map(|composition| {
                        composition
                            .primitive_crates
                            .iter()
                            .map(|primitive_crate| primitive_crate.name.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                false,
                true,
            )
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
    require_supported_target_features(
        target_features,
        &toolchain.artifact_codegen.supported_target_features,
    )
    .map_err(|detail| ForgeError::VerusOutput {
        detail: format!("bound primitive target-feature set is invalid: {detail}"),
    })?;
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
    {
        return Err(ForgeError::VerusOutput {
            detail: "bound toolchain policy or environment whitelist is invalid".to_string(),
        });
    }
    if let (Some(composition), Some(binding)) = (&plan.composition, &receipt.binding.composition) {
        validate_bound_primitive_crates(
            bundle,
            plan.target,
            target_features,
            &toolchain.artifact_codegen.canonical_identity_sha256(),
            &composition.primitive_crates,
            &binding.primitive_crates,
        )?;
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
        (VerifiedTarget::Kernel, Some(model)) => {
            let vstd_dependency = toolchain
                .link_dependencies
                .iter()
                .find(|dependency| dependency.name == "libvstd.rlib")
                .ok_or_else(|| ForgeError::VerusOutput {
                    detail: "kernel model has no bound libvstd.rlib".to_string(),
                })?;
            if model.link_source_name != KERNEL_VSTD_LINK_SOURCE_NAME
                || model.link_source_sha256 != sha256(KERNEL_VSTD_LINK_SOURCE.as_bytes())
                || model.link_build_args != kernel_vstd_link_build_args()
                || if has_machine_boundaries {
                    model.full_rlib_sha256 != vstd_dependency.sha256
                        || vstd_dependency.source_path != model.full_rlib_path
                } else {
                    model.link_rlib_sha256 != vstd_dependency.sha256
                        || vstd_dependency.source_path != "<forge-generated:kernel-vstd-link.rs>"
                }
                || model.source_file_count == 0
                || model.source_total_bytes == 0
                || model.source_sha256.len() != 64
                || model.vir_sha256.len() != 64
                || model.atomic_source_sha256.len() != 64
                || model.full_rlib_sha256.len() != 64
                || Path::new(&model.atomic_source_path).file_name()
                    != Some(std::ffi::OsStr::new("atomic.rs"))
                || Path::new(&model.full_rlib_path).file_name()
                    != Some(std::ffi::OsStr::new("libvstd.rlib"))
            {
                return Err(ForgeError::VerusOutput {
                    detail: "bound kernel vstd model identity is malformed or inconsistent"
                        .to_string(),
                });
            }
            if file_sha256(&bundle.join("evidence").join(&model.link_source_name))?.2
                != model.link_source_sha256
            {
                return Err(ForgeError::VerusOutput {
                    detail: "bound kernel vstd link source has the wrong digest".to_string(),
                });
            }
        }
        (VerifiedTarget::Std, None) => {}
        _ => {
            return Err(ForgeError::VerusOutput {
                detail: "kernel vstd model presence disagrees with the verified target".to_string(),
            });
        }
    }
    validate_machine_model_bundle(bundle, &plan, &toolchain)?;
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
        let current_codegen = collect_codegen_rustc(&current_verus_version, &current_rustup)?;
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
        let replay_kernel_vstd = if matches!(plan.target, VerifiedTarget::Kernel) {
            let current_verus_dir =
                current_verus
                    .parent()
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "replay Verus binary has no installation directory".to_string(),
                    })?;
            let rebuilt =
                build_kernel_vstd_link(&current_verus, current_verus_dir, &replay_environment)?;
            if Some(&rebuilt.2) != toolchain.kernel_vstd_model.as_ref() {
                return Err(ForgeError::VerusOutput {
                    detail:
                        "replay kernel vstd model/source/link identity does not match the receipt"
                            .to_string(),
                });
            }
            Some(rebuilt)
        } else {
            None
        };
        let replay_kernel_vstd_path = replay_kernel_vstd
            .as_ref()
            .map(|(scratch, _, _)| scratch.path.join("libvstd.rlib"));
        let planned_primitive_crates = plan
            .composition
            .as_ref()
            .map(|composition| composition.primitive_crates.as_slice())
            .unwrap_or(&[]);
        let bound_primitive_crates = receipt
            .binding
            .composition
            .as_ref()
            .map(|composition| composition.primitive_crates.as_slice())
            .unwrap_or(&[]);
        let replay_final_vstd_path = if planned_primitive_crates
            .iter()
            .any(|primitive| primitive.proof_basis == "pinned_vstd_machine_model")
        {
            Some(
                current_verus
                    .parent()
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "replay Verus binary has no installation directory".to_string(),
                    })?
                    .join("libvstd.rlib"),
            )
        } else {
            replay_kernel_vstd_path.clone()
        };
        let mut replayed_primitive_crates = Vec::new();
        for (planned, bound) in planned_primitive_crates.iter().zip(bound_primitive_crates) {
            let primitive_source =
                String::from_utf8(file_sha256(&bundle.join(&planned.crate_source_path))?.1)
                    .map_err(|error| ForgeError::VerusOutput {
                        detail: format!(
                            "bound separate primitive crate `{}` source is not UTF-8: {error}",
                            planned.name
                        ),
                    })?;
            let machine_model = planned.proof_basis == "pinned_vstd_machine_model";
            let primitive_vstd_path = if machine_model {
                let path = current_verus
                    .parent()
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "replay Verus binary has no installation directory".to_string(),
                    })?
                    .join("libvstd.rlib");
                let expected = toolchain
                    .kernel_vstd_model
                    .as_ref()
                    .map(|model| model.full_rlib_sha256.as_str())
                    .ok_or_else(|| ForgeError::VerusOutput {
                        detail: "replay machine primitive has no pinned vstd machine model"
                            .to_string(),
                    })?;
                if file_sha256(&path)?.2 != expected {
                    return Err(ForgeError::VerusOutput {
                        detail: format!(
                            "replay full vstd dependency for machine primitive crate `{}` drifted",
                            planned.name
                        ),
                    });
                }
                Some(path)
            } else {
                replay_kernel_vstd_path.clone()
            };
            let replayed = compile_verus_source(CompileVerusInput {
                crate_name: &planned.name,
                source: &primitive_source,
                target: plan.target,
                verus_path: current_verus.to_string_lossy().as_ref(),
                environment: &replay_environment,
                codegen_toolchain_sha256: &current_codegen.canonical_identity_sha256(),
                kernel_vstd_rlib: primitive_vstd_path.as_deref(),
                target_features,
                imports: &[],
                export_vir: true,
                kernel_vstd_model: machine_model,
            })?;
            if !replayed.evidence.success
                || sha256(&replayed.artifact) != bound.rlib_sha256
                || replayed.exported_vir.is_none()
                || replayed.object_members != bound.object_members
            {
                return Err(ForgeError::VerusOutput {
                    detail: format!(
                        "replay did not reproduce separate primitive crate `{}` verified rlib and objects (rlib expected {}, observed {}; exported_interface={}; objects_match={})",
                        planned.name,
                        bound.rlib_sha256,
                        sha256(&replayed.artifact),
                        replayed.exported_vir.is_some(),
                        replayed.object_members == bound.object_members,
                    ),
                });
            }
            replayed_primitive_crates.push(replayed);
        }
        let replay_imports: Vec<VerusImportBytes<'_>> = planned_primitive_crates
            .iter()
            .zip(&replayed_primitive_crates)
            .map(|(planned, replayed)| VerusImportBytes {
                name: &planned.name,
                vir: replayed.exported_vir.as_deref().unwrap_or_default(),
                rlib: &replayed.artifact,
            })
            .collect();
        let compiled = compile_verus_source(CompileVerusInput {
            crate_name: &plan.crate_name,
            source: &source,
            target: plan.target,
            verus_path: current_verus.to_string_lossy().as_ref(),
            environment: &replay_environment,
            codegen_toolchain_sha256: &current_codegen.canonical_identity_sha256(),
            kernel_vstd_rlib: replay_final_vstd_path.as_deref(),
            target_features,
            imports: &replay_imports,
            export_vir: false,
            kernel_vstd_model: true,
        })?;
        if !compiled.evidence.success || sha256(&compiled.artifact) != artifact.sha256 {
            return Err(ForgeError::VerusOutput {
                detail: "replay did not reproduce the bound artifact digest".to_string(),
            });
        }
    }

    Ok(VerifyBuildReport {
        bundle: bundle.to_path_buf(),
        binding_sha256: receipt.binding_sha256,
        replayed: replay,
        artifact_sha256: artifact.sha256.clone(),
    })
}

fn validate_machine_model_bundle(
    bundle: &Path,
    plan: &ArtifactPlanV1,
    toolchain: &ToolchainEvidence,
) -> Result<(), ForgeError> {
    let primitive_crates = plan
        .composition
        .as_ref()
        .map(|composition| composition.primitive_crates.as_slice())
        .unwrap_or(&[]);
    if let Some(invalid) = primitive_crates.iter().find(|primitive| {
        !matches!(
            primitive.proof_basis.as_str(),
            "verus_builtins" | "pinned_vstd_machine_model"
        )
    }) {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "separate primitive crate `{}` has unknown proof basis `{}`",
                invalid.name, invalid.proof_basis
            ),
        });
    }
    let required = primitive_crates
        .iter()
        .any(|primitive| primitive.proof_basis == "pinned_vstd_machine_model");
    let source_path = bundle.join(MACHINE_ATOMIC_MODEL_SOURCE_PATH);
    let rlib_path = bundle.join(MACHINE_VSTD_RLIB_PATH);
    if !required {
        if source_path.exists() {
            return Err(ForgeError::VerusOutput {
                detail: "bundle carries an unrequested machine-model source".to_string(),
            });
        }
        return Ok(());
    }
    let model = toolchain
        .kernel_vstd_model
        .as_ref()
        .ok_or_else(|| ForgeError::VerusOutput {
            detail: "machine primitive bundle has no pinned kernel vstd model evidence".to_string(),
        })?;
    if file_sha256(&source_path)?.2 != model.atomic_source_sha256
        || file_sha256(&rlib_path)?.2 != model.full_rlib_sha256
    {
        return Err(ForgeError::VerusOutput {
            detail: "machine primitive bundle does not bind the exact pinned vstd atomic source and codegen rlib"
                .to_string(),
        });
    }
    Ok(())
}

fn validate_bound_primitive_crates(
    bundle: &Path,
    target: VerifiedTarget,
    target_features: &[String],
    codegen_toolchain_sha256: &str,
    planned: &[PlannedPrimitiveCrateV1],
    bound: &[BoundPrimitiveCrateV1],
) -> Result<(), ForgeError> {
    if planned.len() != bound.len() {
        return Err(ForgeError::VerusOutput {
            detail: "separate primitive crate plan/receipt cardinality mismatch".to_string(),
        });
    }
    for (planned, bound) in planned.iter().zip(bound) {
        let parent = Path::new(&planned.authored_source_path)
            .parent()
            .ok_or_else(|| ForgeError::VerusOutput {
                detail: "separate primitive authored source has no parent".to_string(),
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let expected_vir_path = format!("{parent}/interface.vir");
        let expected_rlib_path = format!("artifact/deps/lib{}.rlib", planned.name);
        let verus_result_path = format!("{parent}/verus-result.json");
        if bound.name != planned.name
            || bound.authored_source_sha256 != planned.authored_source_sha256
            || bound.crate_source_sha256 != planned.crate_source_sha256
            || bound.vir_path != expected_vir_path
            || bound.rlib_path != expected_rlib_path
            || bound.object_members.is_empty()
        {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "separate primitive crate `{}` receipt identity disagrees with its plan",
                    planned.name
                ),
            });
        }
        for path in [&bound.vir_path, &bound.rlib_path, &verus_result_path] {
            validate_relative_path(path)?;
        }
        let (vir_length, _, vir_sha256) = file_sha256(&bundle.join(&bound.vir_path))?;
        let (rlib_length, rlib_bytes, rlib_sha256) = file_sha256(&bundle.join(&bound.rlib_path))?;
        let verus_result_bytes = file_sha256(&bundle.join(&verus_result_path))?.1;
        if vir_length != bound.vir_length
            || vir_sha256 != bound.vir_sha256
            || rlib_length != bound.rlib_length
            || rlib_sha256 != bound.rlib_sha256
            || sha256(&verus_result_bytes) != bound.verus_result_sha256
            || archive_object_members(&rlib_bytes)? != bound.object_members
        {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "separate primitive crate `{}` interface, rlib, object, or proof digest mismatch",
                    planned.name
                ),
            });
        }
        let evidence: VerusEvidence =
            serde_json::from_slice(&verus_result_bytes).map_err(|error| {
                ForgeError::VerusOutput {
                    detail: format!(
                        "invalid separate primitive crate `{}` Verus evidence: {error}",
                        planned.name
                    ),
                }
            })?;
        if !evidence.success
            || evidence.errors != Some(0)
            || evidence.args
                != expected_verus_args(
                    &planned.name,
                    target,
                    target_features,
                    &[],
                    true,
                    planned.proof_basis == "pinned_vstd_machine_model",
                )
            || evidence.source_relative_path != format!("{}.rs", planned.name)
            || evidence.source_sha256_before != planned.crate_source_sha256
            || evidence.source_sha256_after != planned.crate_source_sha256
            || evidence.codegen_toolchain_sha256 != codegen_toolchain_sha256
            || parse_verus_summary(&evidence.stdout) != (true, Some(0))
        {
            return Err(ForgeError::VerusOutput {
                detail: format!(
                    "separate primitive crate `{}` does not carry exact no-cheating proof/codegen evidence",
                    planned.name
                ),
            });
        }
    }
    Ok(())
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

    fn package_fixture(
        roots: &[&str],
        api_imports: &[&str],
        base_source: &str,
        api_source: &str,
    ) -> (ScratchTree, PathBuf) {
        let tree = ScratchTree::new_in_temp("verified_package_fixture").unwrap();
        fs::create_dir(tree.path.join("src")).unwrap();
        fs::write(tree.path.join("src/base.th"), base_source).unwrap();
        fs::write(tree.path.join("src/api.th"), api_source).unwrap();
        let manifest = thermite_package::PackageManifestV1 {
            schema: thermite_package::PACKAGE_SCHEMA.to_string(),
            name: "package_fixture".to_string(),
            roots: roots.iter().map(|root| (*root).to_string()).collect(),
            modules: vec![
                thermite_package::PackageModuleV1 {
                    name: "api".to_string(),
                    path: "src/api.th".to_string(),
                    imports: api_imports
                        .iter()
                        .map(|import| (*import).to_string())
                        .collect(),
                },
                thermite_package::PackageModuleV1 {
                    name: "base".to_string(),
                    path: "src/base.th".to_string(),
                    imports: Vec::new(),
                },
            ],
        };
        let mut bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        bytes.push(b'\n');
        let path = tree.path.join("fixture.thpkg.json");
        fs::write(&path, bytes).unwrap();
        (tree, path)
    }

    fn parse(source: &str) -> Program {
        let parsed = thermite_syntax::parse(source);
        assert!(parsed.is_clean(), "{:?}", parsed.errors);
        parsed.program
    }

    #[test]
    fn package_resolution_requires_declared_cross_module_calls() {
        let base = "fn base(x: u64) -> u64 req true ens result == x fx pure { x }\n";
        let api = "fn api(x: u64) -> u64 req true ens result == x fx pure { base(x) }\n";
        let (_tree, path) = package_fixture(&["api", "base"], &[], base, api);
        let error = match prepare_thermite_input(&path) {
            Ok(_) => panic!("undeclared cross-module call was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("without declaring that import"), "{error}");

        let (_tree, path) = package_fixture(&["api"], &["base"], base, api);
        assert!(prepare_thermite_input(&path).is_ok());
    }

    #[test]
    fn package_resolution_requires_declared_cross_module_signature_types() {
        let base = "struct Token { value: u64 }\n";
        let api = "fn token_value(token: Token) -> u64 req true ens result == token.value fx pure { token.value }\n";
        let (_tree, path) = package_fixture(&["api", "base"], &[], base, api);
        let error = match prepare_thermite_input(&path) {
            Ok(_) => panic!("undeclared cross-module signature type was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("uses `Token`"), "{error}");
        assert!(error.contains("without declaring that import"), "{error}");
    }

    #[test]
    fn package_resolution_requires_declared_cross_module_capacity_constants() {
        let base = "const CAP: usize = 4;\n";
        let api = r#"fn api(at: usize) -> u64
  req at < CAP
  ens result == 0
  fx pure
{
  let slots: [u64; CAP] = [0; CAP];
  slots[at]
}
"#;
        let (_tree, path) = package_fixture(&["api", "base"], &[], base, api);
        let error = match prepare_thermite_input(&path) {
            Ok(_) => panic!("undeclared cross-module capacity constant was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("uses `CAP`"), "{error}");
        assert!(error.contains("without declaring that import"), "{error}");

        let (_tree, path) = package_fixture(&["api"], &["base"], base, api);
        assert!(prepare_thermite_input(&path).is_ok());
    }

    #[test]
    fn package_opaque_construction_is_limited_to_the_defining_module() {
        let base = r#"#[opaque] struct State { value: u64 }
fn state_new(value: u64) -> State
  req true
  ens result.value == value
  fx pure
{ State { value: value } }
"#;
        let foreign_literal = r#"fn forge_state(value: u64) -> State
  req true
  ens result.value == value
  fx pure
{ State { value: value } }
"#;
        let (_tree, path) = package_fixture(&["api"], &["base"], base, foreign_literal);
        let error = match prepare_thermite_input(&path) {
            Ok(_) => panic!("a foreign module constructed an opaque type"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("module `api` constructs `#[opaque]` type `State`"),
            "{error}"
        );
        assert!(error.contains("declared in module `base`"), "{error}");

        let through_constructor = r#"fn api(value: u64) -> State
  req true
  ens true
  fx pure
{ state_new(value) }
"#;
        let (_tree, path) = package_fixture(&["api"], &["base"], base, through_constructor);
        assert!(prepare_thermite_input(&path).is_ok());
    }

    #[test]
    fn package_opaque_field_reads_and_writes_are_limited_to_the_defining_module() {
        let base = r#"#[opaque] struct State { value: u64 }
fn state_new(value: u64) -> State
  req true ens result.value == value fx pure
{ State { value: value } }
"#;
        for (label, api) in [
            (
                "contract read",
                r#"fn inspect(state: State) -> u64
  req true ens result == state.value fx pure
{ 0 }
"#,
            ),
            (
                "body read",
                r#"fn inspect(state: State) -> u64
  req true ens true fx pure
{ state.value }
"#,
            ),
            (
                "body write",
                r#"fn change(state: &mut State, value: u64) -> ()
  req true ens true fx pure
{ state.value = value; }
"#,
            ),
            (
                "constructor-chain read",
                r#"fn inspect(value: u64) -> u64
  req true ens true fx pure
{ state_new(value).value }
"#,
            ),
            (
                "inferred-local read",
                r#"fn inspect(value: u64) -> u64
  req true ens true fx pure
{ let state = state_new(value); state.value }
"#,
            ),
        ] {
            let (_tree, path) = package_fixture(&["api"], &["base"], base, api);
            let error = match prepare_thermite_input(&path) {
                Ok(_) => panic!("foreign opaque {label} was accepted"),
                Err(error) => error.to_string(),
            };
            assert!(
                error.contains("module `api` accesses a field of `#[opaque]` type `State`"),
                "{label}: {error}"
            );
            assert!(error.contains("declared in module `base`"), "{error}");
        }

        let unresolved_pattern = r#"fn inspect(state: Option<State>) -> u64
  req true ens true fx pure
{ match state { Some(value) => value.value, None => 0 } }
"#;
        let (_tree, path) = package_fixture(&["api"], &["base"], base, unresolved_pattern);
        let error = match prepare_thermite_input(&path) {
            Ok(_) => panic!("an unresolved foreign opaque pattern projection was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("module `api` accesses field `value`"),
            "{error}"
        );
        assert!(
            error.contains("foreign `#[opaque]` type(s) State"),
            "{error}"
        );

        let unrelated_plain = r#"struct Public { value: u64 }
fn inspect(public: Public) -> u64
  req true ens result == public.value fx pure
{ public.value }
"#;
        let (_tree, path) = package_fixture(&["api"], &["base"], base, unrelated_plain);
        assert!(
            prepare_thermite_input(&path).is_ok(),
            "a type-resolved plain field sharing an opaque field name remains legal"
        );
    }

    #[test]
    fn opaque_single_file_construction_remains_the_defining_module() {
        let program = parse(
            r#"#[opaque] struct State { value: u64 }
fn state_new(value: u64) -> State
  req true
  ens result.value == value
  fx pure
{ State { value: value } }
"#,
        );
        assert!(thermite_spec::validate(&program).is_ok());
    }

    #[test]
    fn package_exports_must_be_declared_by_root_modules() {
        let base = "fn base(x: u64) -> u64 req true ens result == x fx pure { x }\n";
        let api = "fn api(x: u64) -> u64 req true ens result == x fx pure { base(x) }\n";
        let (_tree, path) = package_fixture(&["api"], &["base"], base, api);
        let prepared = prepare_thermite_input(&path).unwrap();
        let package = prepared.package.as_ref().unwrap();
        assert!(package_exports_are_roots(package, &["api".to_string()]).is_ok());
        let error = package_exports_are_roots(package, &["base".to_string()]).unwrap_err();
        assert!(error.contains("non-root module `base`"), "{error}");
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
            supported_target_features: vec!["sse2".to_string()],
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
            package: None,
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
    fn public_abi_admits_only_finite_plain_record_values() {
        let plain = parse(
            "struct Stamp { words: [u64; 2], flags: (bool, u8) } \
             struct Slot { stamp: Stamp, owner: usize } \
             fn equal(left: [Slot; 4], right: [Slot; 4]) -> bool \
             req true ens true fx pure { true }",
        );
        let exports = plan_exports(
            &plain,
            &["equal".to_string()],
            "plain_records",
            VerifiedTarget::Std,
            "x86_64-unknown-linux-gnu",
            "64",
            "little",
        )
        .expect("finite plain record arrays belong to the verified Rust ABI");
        assert_eq!(exports[0].parameter_types, ["[Slot;4]", "[Slot;4]"]);

        for source in [
            "#[sealed] struct Token { raw: u64 } \
             fn expose(value: [Token; 2]) -> bool req true ens true fx pure { true }",
            "#[opaque] struct State { raw: u64 } \
             fn expose(value: [State; 2]) -> bool req true ens true fx pure { true }",
            "struct Label { text: String } \
             fn expose(value: [Label; 2]) -> bool req true ens true fx pure { true }",
        ] {
            let hidden = parse(source);
            let error = plan_exports(
                &hidden,
                &["expose".to_string()],
                "hidden_records",
                VerifiedTarget::Std,
                "x86_64-unknown-linux-gnu",
                "64",
                "little",
            )
            .expect_err("authority-bearing or heap-backed record ABI must fail closed");
            assert!(
                error.contains("outside the verified public Rust ABI"),
                "{error}"
            );
        }
    }

    #[test]
    fn public_abi_admits_direct_opaque_record_lifecycle_roots() {
        let program = parse(
            "#[opaque] struct State { generation: u64, occupied: bool } \
             fn advance(state: &mut State, next: u64) -> bool \
             req true \
             ens result == old(state).occupied \
             ens final(state).generation == next \
             ens final(state).occupied == old(state).occupied \
             fx pure { \
               let previous: bool = state.occupied; \
               state.generation = next; previous \
             }",
        );
        let exports = plan_exports(
            &program,
            &["advance".to_string()],
            "opaque_lifecycle",
            VerifiedTarget::Std,
            "x86_64-unknown-linux-gnu",
            "64",
            "little",
        )
        .expect("a direct finite opaque record borrow belongs to the verified ABI");
        assert_eq!(exports[0].parameter_types, ["&mut State", "u64"]);
        assert_eq!(exports[0].ownership, ["exclusive_borrow", "by_value"]);
        assert!(exports[0]
            .signature
            .contains("fn advance(state:&mut State,next:u64)->bool"));
    }

    #[test]
    fn public_abi_fingerprint_binds_record_layout_and_resolved_capacities() {
        let fingerprint = |source: &str| {
            let program = parse(source);
            plan_exports(
                &program,
                &["expose".to_string()],
                "layout_bound",
                VerifiedTarget::Std,
                "x86_64-unknown-linux-gnu",
                "64",
                "little",
            )
            .expect("finite plain layout should plan")
            .remove(0)
            .abi_sha256
        };
        let base = fingerprint(
            "const CAP: usize = 4; \
             struct Slot { owner: usize, flags: (bool, u8) } \
             fn expose(values: [Slot; CAP]) -> bool \
             req true ens true fx pure { true }",
        );
        let changed_capacity = fingerprint(
            "const CAP: usize = 8; \
             struct Slot { owner: usize, flags: (bool, u8) } \
             fn expose(values: [Slot; CAP]) -> bool \
             req true ens true fx pure { true }",
        );
        let changed_field = fingerprint(
            "const CAP: usize = 4; \
             struct Slot { owner: u64, flags: (bool, u8) } \
             fn expose(values: [Slot; CAP]) -> bool \
             req true ens true fx pure { true }",
        );
        let reordered_fields = fingerprint(
            "const CAP: usize = 4; \
             struct Slot { flags: (bool, u8), owner: usize } \
             fn expose(values: [Slot; CAP]) -> bool \
             req true ens true fx pure { true }",
        );

        assert_ne!(base, changed_capacity, "resolved capacity is ABI data");
        assert_ne!(base, changed_field, "transitive field type is ABI data");
        assert_ne!(base, reordered_fields, "record field order is ABI data");
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
        let plain_state = parse("struct State { value: u64 }");
        let opaque_state = parse("#[opaque] struct State { value: u64 }");
        let sealed_state = parse("#[sealed] struct State { value: u64 }");
        assert_eq!(
            normalized_program_sha256(&compact),
            normalized_program_sha256(&presented_differently)
        );
        assert_ne!(
            normalized_program_sha256(&compact),
            normalized_program_sha256(&changed)
        );
        assert_ne!(
            normalized_program_sha256(&plain_state),
            normalized_program_sha256(&opaque_state),
            "the normalized proof/build identity must bind the opaque barrier"
        );
        assert_ne!(
            normalized_program_sha256(&opaque_state),
            normalized_program_sha256(&sealed_state),
            "opaque construction and sealed boundary-only minting are distinct semantics"
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
        for field in 0..19 {
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
                14 => changed.supported_target_features.push("xsave".to_string()),
                15 => changed.target_libdir_sha256 = "9".repeat(64),
                16 => changed.target_libdir_file_count += 1,
                17 => changed.target_libdir_total_bytes += 1,
                18 => changed.linker_identity.push_str(" changed"),
                _ => unreachable!(),
            }
            assert_ne!(digest, changed.canonical_identity_sha256(), "field {field}");
        }
    }

    #[test]
    fn rustc_target_feature_inventory_is_parsed_canonically() {
        let parsed = parse_rustc_target_features(
            "Features supported by rustc for this target:\n    sse2 - SSE2.\n    aes - AES.\n",
        )
        .unwrap();
        assert_eq!(parsed, ["aes", "sse2"]);
        assert!(parse_rustc_target_features("header only\n").is_err());
        assert!(parse_rustc_target_features("  +sse2 - invalid\n").is_err());
        assert!(require_supported_target_features(
            &["sse2".to_string()],
            &["aes".to_string(), "sse2".to_string()]
        )
        .is_ok());
        assert!(require_supported_target_features(
            &["imaginary".to_string()],
            &["aes".to_string(), "sse2".to_string()]
        )
        .unwrap_err()
        .contains("not in the pinned codegen rustc"));
    }
}
