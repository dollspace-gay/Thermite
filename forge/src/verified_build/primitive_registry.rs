//! Consumer-owned frozen primitive registry planning.
//!
//! The registry does not define a platform operation table.  It closes exactly
//! the `#[boundary]` functions reachable from the selected Thermite roots and
//! resolves each one to a checked function in an exact direct-Verus shell.

use super::*;

const REGISTRY_SCHEMA_V1: &str = "thermite.frozen-primitive-registry.v1";
const REGISTRY_SCHEMA_V2: &str = "thermite.frozen-primitive-registry.v2";
const REGISTRY_SCHEMA_V3: &str = "thermite.frozen-primitive-registry.v3";
const REGISTRY_EVIDENCE_PATH: &str = "evidence/frozen-primitive-registry.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PrimitiveRegistrySource {
    pub(super) bytes: Vec<u8>,
    pub(super) plan: PlannedPrimitiveRegistryV1,
    pub(super) bindings: Vec<thermite_lower::L3BoundaryBinding>,
    pub(super) separate_crates: BTreeSet<String>,
    pub(super) machine_crates: BTreeSet<String>,
    pub(super) machine_boundaries: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryDocument {
    schema: String,
    target: RegistryTarget,
    entries: Vec<RegistryEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryTarget {
    target_triple: String,
    target_features: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryEntry {
    semantic_name: String,
    version: u64,
    required: bool,
    thermite_name: String,
    boundary_target: String,
    signature: String,
    contract_sha256: String,
    effects_sha256: String,
    effects: Vec<String>,
    ownership: RegistryOwnership,
    implementation: RegistryImplementation,
    model: String,
    refinement: String,
    proof_obligations: Vec<String>,
    concurrency: String,
    memory_orderings: Vec<String>,
    failure: String,
    #[serde(default)]
    machine: Option<RegistryMachine>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryMachine {
    family: String,
    operation: String,
    residual_assumptions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryOwnership {
    parameters: Vec<String>,
    result: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryImplementation {
    shell_module: String,
    item: String,
    source_sha256: String,
    #[serde(default)]
    linkage: Option<String>,
    abi: String,
    symbol: String,
    alignment: u64,
}

pub(super) fn load_from_evidence(
    bytes: Vec<u8>,
    program: &Program,
    closure: &VerifiedClosure,
    shells: &[DirectVerusSource],
    target_triple: &str,
) -> Result<PrimitiveRegistrySource, String> {
    plan_from_bytes(bytes, program, closure, shells, target_triple)
}

fn plan_from_bytes(
    bytes: Vec<u8>,
    program: &Program,
    closure: &VerifiedClosure,
    shells: &[DirectVerusSource],
    target_triple: &str,
) -> Result<PrimitiveRegistrySource, String> {
    let document: RegistryDocument = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid frozen primitive registry JSON: {error}"))?;
    if !matches!(
        document.schema.as_str(),
        REGISTRY_SCHEMA_V1 | REGISTRY_SCHEMA_V2 | REGISTRY_SCHEMA_V3
    ) {
        return Err(format!(
            "unsupported frozen primitive registry schema `{}`",
            document.schema
        ));
    }
    let registry_v2 = document.schema == REGISTRY_SCHEMA_V2;
    let registry_v3 = document.schema == REGISTRY_SCHEMA_V3;
    if document.target.target_triple != target_triple {
        return Err(format!(
            "primitive registry target triple `{}` does not match codegen target `{target_triple}`",
            document.target.target_triple
        ));
    }
    require_sorted_unique(
        "target features",
        &document.target.target_features,
        valid_feature,
    )?;
    if document.entries.is_empty() {
        return Err("a frozen primitive registry must declare at least one entry".to_string());
    }
    if document
        .entries
        .windows(2)
        .any(|pair| pair[0].semantic_name >= pair[1].semantic_name)
    {
        return Err(
            "frozen primitive registry entries must be strictly sorted by semantic_name"
                .to_string(),
        );
    }

    let sealed: BTreeSet<&str> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item) if item.sealed => Some(item.name.as_str()),
            _ => None,
        })
        .collect();
    let reachable_boundaries: BTreeSet<&str> = closure
        .functions
        .iter()
        .filter_map(|name| {
            program.items.iter().find_map(|item| match item {
                Item::Fn(function) if function.name == *name && function.boundary.is_some() => {
                    Some(function.name.as_str())
                }
                _ => None,
            })
        })
        .collect();
    if reachable_boundaries.is_empty() {
        return Err(
            "a primitive registry was supplied but the selected Thermite closure reaches no #[boundary] function"
                .to_string(),
        );
    }

    let shell_by_name: BTreeMap<&str, &DirectVerusSource> = shells
        .iter()
        .map(|shell| (shell.plan.name.as_str(), shell))
        .collect();
    let mut semantic_names = BTreeSet::new();
    let mut thermite_names = BTreeSet::new();
    let mut boundary_targets = BTreeSet::new();
    let mut implementations = BTreeSet::new();
    let mut rows = Vec::new();
    let mut bindings = Vec::new();
    let mut module_linkages = BTreeMap::<String, String>::new();
    let mut separate_crates = BTreeSet::new();
    let mut machine_crates = BTreeSet::new();
    let mut machine_boundaries = BTreeSet::new();

    for entry in document.entries {
        if !valid_semantic_name(&entry.semantic_name, entry.version) {
            return Err(format!(
                "primitive semantic name `{}` is not canonical or does not end in `@v{}`",
                entry.semantic_name, entry.version
            ));
        }
        if !semantic_names.insert(entry.semantic_name.clone()) {
            return Err(format!(
                "duplicate primitive semantic name `{}`",
                entry.semantic_name
            ));
        }
        if !thermite_names.insert(entry.thermite_name.clone()) {
            return Err(format!(
                "duplicate primitive Thermite binding `{}`",
                entry.thermite_name
            ));
        }
        if !boundary_targets.insert(entry.boundary_target.clone()) {
            return Err(format!(
                "duplicate primitive boundary target `{}`",
                entry.boundary_target
            ));
        }
        if !implementations.insert((
            entry.implementation.shell_module.clone(),
            entry.implementation.item.clone(),
        )) {
            return Err(format!(
                "duplicate primitive implementation `{}::{}`",
                entry.implementation.shell_module, entry.implementation.item
            ));
        }

        let function = program.items.iter().find_map(|item| match item {
            Item::Fn(function) if function.name == entry.thermite_name => Some(function),
            _ => None,
        });
        let Some(function) = function else {
            return Err(format!(
                "primitive entry `{}` names unknown Thermite function `{}`",
                entry.semantic_name, entry.thermite_name
            ));
        };
        let Some(boundary) = function.boundary.as_ref() else {
            return Err(format!(
                "primitive entry `{}` names `{}`, which is not a #[boundary] declaration",
                entry.semantic_name, entry.thermite_name
            ));
        };
        if boundary.target != entry.boundary_target {
            return Err(format!(
                "primitive entry `{}` boundary target drift: registry `{}`, Thermite `{}`",
                entry.semantic_name, entry.boundary_target, boundary.target
            ));
        }
        let reachable = reachable_boundaries.contains(function.name.as_str());
        if entry.required && !reachable {
            return Err(format!(
                "required primitive entry `{}` is unreachable from the selected roots",
                entry.semantic_name
            ));
        }

        let signature = function_signature(function)?;
        if entry.signature != signature {
            return Err(format!(
                "primitive entry `{}` signature drift: expected `{signature}`, found `{}`",
                entry.semantic_name, entry.signature
            ));
        }
        let contract_sha256 = semantic_contract_sha256(function);
        require_digest(
            &entry.semantic_name,
            "contract_sha256",
            &entry.contract_sha256,
            &contract_sha256,
        )?;
        let effects_sha256 = sha256(format!("{:#?}", function.contract.fx).as_bytes());
        require_digest(
            &entry.semantic_name,
            "effects_sha256",
            &entry.effects_sha256,
            &effects_sha256,
        )?;
        let effects = effect_spellings(&function.contract.fx);
        if entry.effects != effects {
            return Err(format!(
                "primitive entry `{}` effect-row drift: expected {:?}, found {:?}",
                entry.semantic_name, effects, entry.effects
            ));
        }

        let parameter_ownership: Vec<String> = function
            .params
            .iter()
            .map(|parameter| parameter_ownership(&parameter.ty, &sealed))
            .collect();
        let result_ownership = result_ownership(&function.ret, &sealed)?;
        if entry.ownership.parameters != parameter_ownership
            || entry.ownership.result != result_ownership
        {
            return Err(format!(
                "primitive entry `{}` ownership drift: expected parameters {:?}, result `{}`, found parameters {:?}, result `{}`",
                entry.semantic_name,
                parameter_ownership,
                result_ownership,
                entry.ownership.parameters,
                entry.ownership.result
            ));
        }

        let linkage = entry
            .implementation
            .linkage
            .as_deref()
            .unwrap_or("same_crate");
        if (registry_v2 || registry_v3) && entry.implementation.linkage.is_none() {
            return Err(format!(
                "primitive entry `{}` in registry v2/v3 must declare implementation.linkage",
                entry.semantic_name
            ));
        }
        if !registry_v2 && !registry_v3 && entry.implementation.linkage.is_some() {
            return Err(format!(
                "primitive entry `{}` cannot declare implementation.linkage in registry v1",
                entry.semantic_name
            ));
        }
        if !matches!(
            linkage,
            "same_crate" | "separate_verus_crate" | "separate_verus_machine_crate"
        ) {
            return Err(format!(
                "primitive entry `{}` has unknown implementation linkage `{linkage}`",
                entry.semantic_name
            ));
        }
        if linkage == "separate_verus_crate" && !registry_v2 && !registry_v3 {
            return Err(format!(
                "primitive entry `{}` requires frozen primitive registry v2 for separate-crate linkage",
                entry.semantic_name
            ));
        }
        if linkage == "separate_verus_machine_crate" && !registry_v3 {
            return Err(format!(
                "primitive entry `{}` requires frozen primitive registry v3 for machine-crate linkage",
                entry.semantic_name
            ));
        }
        if let Some(previous) = module_linkages.insert(
            entry.implementation.shell_module.clone(),
            linkage.to_string(),
        ) {
            if previous != linkage {
                return Err(format!(
                    "direct-Verus module `{}` cannot mix `{previous}` and `{linkage}` linkage",
                    entry.implementation.shell_module
                ));
            }
        }

        let Some(shell) = shell_by_name.get(entry.implementation.shell_module.as_str()) else {
            return Err(format!(
                "primitive entry `{}` names unknown direct-Verus shell `{}`",
                entry.semantic_name, entry.implementation.shell_module
            ));
        };
        let item = shell
            .plan
            .items
            .iter()
            .find(|item| item.kind == "fn" && item.name == entry.implementation.item);
        let Some(item) = item else {
            return Err(format!(
                "primitive entry `{}` implementation `{}::{}` is not an inventoried checked function",
                entry.semantic_name,
                entry.implementation.shell_module,
                entry.implementation.item
            ));
        };
        if item.visibility != "public" {
            return Err(format!(
                "primitive implementation `{}::{}` must be public to the generated checked wrapper",
                entry.implementation.shell_module, entry.implementation.item
            ));
        }
        require_digest(
            &entry.semantic_name,
            "implementation.source_sha256",
            &entry.implementation.source_sha256,
            &shell.plan.sha256,
        )?;
        if entry.implementation.abi != "Rust" {
            return Err(format!(
                "primitive entry `{}` requests ABI `{}`; frozen primitive registry exact refinement supports only `Rust`",
                entry.semantic_name, entry.implementation.abi
            ));
        }
        let call_target = format!(
            "{}::{}",
            entry.implementation.shell_module, entry.implementation.item
        );
        if entry.implementation.symbol != call_target {
            return Err(format!(
                "primitive entry `{}` implementation symbol must be the exact checked call target `{call_target}`",
                entry.semantic_name
            ));
        }
        if entry.implementation.alignment == 0
            || !entry.implementation.alignment.is_power_of_two()
            || entry.implementation.alignment > 1_048_576
        {
            return Err(format!(
                "primitive entry `{}` alignment must be a power of two in 1..=1048576",
                entry.semantic_name
            ));
        }
        let expected_refinement = match linkage {
            "same_crate" => "same_crate_verus_checked_wrapper",
            "separate_verus_crate" => "separate_crate_verus_import",
            "separate_verus_machine_crate" => "separate_crate_verus_machine_import",
            _ => unreachable!(),
        };
        let expected_model = if linkage == "separate_verus_machine_crate" {
            "pinned_vstd_atomic_permission"
        } else {
            "thermite_contract"
        };
        if entry.model != expected_model || entry.refinement != expected_refinement {
            return Err(format!(
                "primitive entry `{}` with `{linkage}` linkage must use model `{expected_model}` and refinement `{expected_refinement}`",
                entry.semantic_name,
            ));
        }
        require_sorted_unique(
            "proof obligations",
            &entry.proof_obligations,
            valid_evidence_name,
        )?;
        let mandatory_obligations: &[&str] = if linkage == "same_crate" {
            &[
                "contract_refinement",
                "exact_implementation_call",
                "whole_crate_no_cheating",
            ]
        } else if linkage == "separate_verus_crate" {
            &[
                "contract_refinement",
                "exact_implementation_call",
                "exported_verus_interface",
                "imported_call_refinement",
                "separate_object_identity",
                "separate_source_identity",
                "whole_crate_no_cheating",
            ]
        } else {
            &[
                "atomic_permission_refinement",
                "contract_refinement",
                "exact_implementation_call",
                "exported_verus_interface",
                "imported_call_refinement",
                "memory_ordering_refinement",
                "pinned_machine_model",
                "separate_object_identity",
                "separate_source_identity",
                "whole_crate_no_cheating",
            ]
        };
        for required in mandatory_obligations {
            if !entry
                .proof_obligations
                .iter()
                .any(|obligation| obligation == required)
            {
                return Err(format!(
                    "primitive entry `{}` omits mandatory proof obligation `{required}`",
                    entry.semantic_name
                ));
            }
        }
        let Some(declared_class) = MachineClass::declared(entry.concurrency.as_str()) else {
            return Err(format!(
                "primitive entry `{}` has unknown concurrency semantics `{}`",
                entry.semantic_name, entry.concurrency
            ));
        };
        require_sorted_unique(
            "memory orderings",
            &entry.memory_orderings,
            valid_memory_ordering,
        )?;
        // The effective machine class is the maximum of the class the source
        // effect row implies and the class the entry declares for itself
        // (`.design/build/frozen-primitive-registry.md` §"Source-derived
        // minimum machine class"). A declaration raises the class and leaves
        // the source-derived floor in place.
        let (source_class, source_atom) = source_machine_class(&function.contract.fx);
        let effective_class = source_class.max(declared_class);
        if linkage != "separate_verus_machine_crate" && effective_class != MachineClass::Sequential
        {
            return Err(format!(
                "primitive entry `{}` binds Thermite function `{}`, whose effective machine class is `{}` from its {}, and safe frozen primitive registry linkages can directly refine only sequential safe-Rust operations; atomic, volatile, and privileged implementations require registry v3 machine evidence",
                entry.semantic_name,
                function.name,
                effective_class.surface(),
                effective_class_basis(source_class, source_atom, declared_class, &entry.concurrency)
            ));
        }
        if entry.concurrency == "sequential" && !entry.memory_orderings.is_empty() {
            return Err(format!(
                "sequential primitive entry `{}` cannot declare memory orderings",
                entry.semantic_name
            ));
        }
        if linkage == "separate_verus_machine_crate" && effective_class != MachineClass::Atomic {
            return Err(format!(
                "primitive entry `{}` binds Thermite function `{}`, whose effective machine class is `{}` from its {}, and frozen primitive registry v3 binds machine evidence for the atomic class only",
                entry.semantic_name,
                function.name,
                effective_class.surface(),
                effective_class_basis(source_class, source_atom, declared_class, &entry.concurrency)
            ));
        }
        if linkage == "separate_verus_machine_crate" {
            validate_machine_entry(&entry, function, shell)?;
            machine_crates.insert(entry.implementation.shell_module.clone());
            if reachable {
                machine_boundaries.insert(function.name.clone());
            }
        } else if entry.machine.is_some() {
            return Err(format!(
                "primitive entry `{}` may declare machine evidence only with separate_verus_machine_crate linkage",
                entry.semantic_name
            ));
        }
        if !matches!(
            entry.failure.as_str(),
            "total" | "returns_error" | "terminal"
        ) {
            return Err(format!(
                "primitive entry `{}` has unknown failure behavior `{}`",
                entry.semantic_name, entry.failure
            ));
        }

        if reachable {
            bindings.push(thermite_lower::L3BoundaryBinding {
                source_name: function.name.clone(),
                call_target,
            });
        }
        if matches!(
            linkage,
            "separate_verus_crate" | "separate_verus_machine_crate"
        ) {
            separate_crates.insert(entry.implementation.shell_module.clone());
        }
        let (machine_family, machine_operation, residual_assumptions) = entry
            .machine
            .map(|machine| {
                (
                    Some(machine.family),
                    Some(machine.operation),
                    machine.residual_assumptions,
                )
            })
            .unwrap_or_else(|| (None, None, Vec::new()));
        rows.push(PlannedPrimitiveEntryV1 {
            semantic_name: entry.semantic_name,
            version: entry.version,
            required: entry.required,
            reachable,
            thermite_name: function.name.clone(),
            semantic_address: format!("fn::{}", function.name),
            boundary_target: boundary.target.clone(),
            signature,
            contract_sha256,
            effects_sha256,
            effects,
            parameter_ownership,
            result_ownership,
            implementation_shell: entry.implementation.shell_module,
            implementation_item: entry.implementation.item,
            implementation_source_sha256: shell.plan.sha256.clone(),
            implementation_linkage: linkage.to_string(),
            implementation_abi: entry.implementation.abi,
            implementation_symbol: entry.implementation.symbol,
            alignment: entry.implementation.alignment,
            model: entry.model,
            refinement: entry.refinement,
            proof_obligations: entry.proof_obligations,
            concurrency: entry.concurrency,
            memory_orderings: entry.memory_orderings,
            failure: entry.failure,
            machine_family,
            machine_operation,
            residual_assumptions,
        });
    }

    for boundary in &reachable_boundaries {
        if !rows
            .iter()
            .any(|entry| entry.reachable && entry.thermite_name == **boundary)
        {
            return Err(format!(
                "reachable Thermite boundary `{boundary}` has no frozen primitive registry entry"
            ));
        }
    }
    bindings.sort_by(|left, right| left.source_name.cmp(&right.source_name));
    Ok(PrimitiveRegistrySource {
        plan: PlannedPrimitiveRegistryV1 {
            schema: document.schema,
            path: REGISTRY_EVIDENCE_PATH.to_string(),
            length: bytes.len() as u64,
            sha256: sha256(&bytes),
            target: PlannedPrimitiveTargetV1 {
                target_triple: document.target.target_triple,
                target_features: document.target.target_features,
            },
            entries: rows,
        },
        bytes,
        bindings,
        separate_crates,
        machine_crates,
        machine_boundaries,
    })
}

fn validate_machine_entry(
    entry: &RegistryEntry,
    function: &thermite_syntax::FnItem,
    shell: &DirectVerusSource,
) -> Result<(), String> {
    let Some(machine) = entry.machine.as_ref() else {
        return Err(format!(
            "machine primitive entry `{}` must declare machine evidence",
            entry.semantic_name
        ));
    };
    require_sorted_unique(
        "residual assumptions",
        &machine.residual_assumptions,
        valid_evidence_name,
    )?;
    let expected_assumptions = [
        "hardware_atomic_semantics",
        "pinned_vstd_external_body",
        "rust_core_atomic_codegen",
    ];
    if machine.family != "atomic"
        || machine.operation != "p_atomic_u64_seq_cst_roundtrip"
        || entry.concurrency != "atomic"
        || entry.memory_orderings != ["seq_cst"]
        || machine.residual_assumptions != expected_assumptions.map(str::to_string)
    {
        return Err(format!(
            "machine primitive entry `{}` is not the admitted atomic u64 SeqCst roundtrip model with its exact residual assumptions",
            entry.semantic_name
        ));
    }
    if function.params.len() != 1
        || function.params[0].name != "value"
        || function.params[0].ty != Type::Prim(PrimType::U64)
        || function.ret != Type::Prim(PrimType::U64)
    {
        return Err(format!(
            "machine primitive entry `{}` must have the pilot signature fn name(value:u64)->u64",
            entry.semantic_name
        ));
    }
    let expected = format!(
        "pub fn {}(value: u64) -> (result: u64)\n    ensures result == value,\n{{\n    let (atomic, Tracked(permission)) = vstd::atomic::PAtomicU64::new(value);\n    atomic.load(Tracked(&permission))\n}}\n",
        entry.implementation.item
    );
    if shell.bytes != expected.as_bytes() {
        return Err(format!(
            "machine primitive entry `{}` implementation must equal the canonical pinned-vstd atomic u64 SeqCst roundtrip adapter",
            entry.semantic_name
        ));
    }
    Ok(())
}

fn function_signature(function: &thermite_syntax::FnItem) -> Result<String, String> {
    let params = function
        .params
        .iter()
        .map(|parameter| {
            Ok(format!(
                "{}:{}",
                parameter.name,
                super::composition::type_spelling(&parameter.ty)?
            ))
        })
        .collect::<Result<Vec<_>, String>>()?
        .join(",");
    Ok(format!(
        "fn {}({params})->{}",
        function.name,
        super::composition::type_spelling(&function.ret)?
    ))
}

fn semantic_contract_sha256(function: &thermite_syntax::FnItem) -> String {
    let debug = format!("{:#?}:{:#?}", function.contract, function.dec);
    sha256(normalize_program_debug(&debug).as_bytes())
}

fn effect_spellings(row: &EffectRow) -> Vec<String> {
    match row {
        EffectRow::Pure => vec!["pure".to_string()],
        EffectRow::Set(effects) => effects
            .iter()
            .map(|effect| match effect {
                Effect::Read(path) => format!("read({path})"),
                Effect::Write(path) => format!("write({path})"),
                Effect::Net(domain) => format!("net({domain})"),
                Effect::Alloc => "alloc".to_string(),
                Effect::Time => "time".to_string(),
                Effect::Rand => "rand".to_string(),
                Effect::Panic => "panic".to_string(),
                Effect::Diverge => "diverge".to_string(),
                Effect::Term => "term".to_string(),
                Effect::Platform(domain) => format!("platform({})", domain.surface()),
            })
            .collect(),
    }
}

/// The four machine classes the registry `concurrency` vocabulary names, in the
/// order `sequential < volatile < atomic < privileged` that
/// `.design/build/frozen-primitive-registry.md` §"Source-derived minimum
/// machine class" fixes. The derived `Ord` is that order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MachineClass {
    Sequential,
    Volatile,
    Atomic,
    Privileged,
}

impl MachineClass {
    /// The registry surface spelling of a class.
    fn surface(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Volatile => "volatile",
            Self::Atomic => "atomic",
            Self::Privileged => "privileged",
        }
    }

    /// The class an entry declares for itself. The vocabulary is the closed set
    /// the schema accepts, so any other spelling has no class.
    fn declared(concurrency: &str) -> Option<Self> {
        match concurrency {
            "sequential" => Some(Self::Sequential),
            "volatile" => Some(Self::Volatile),
            "atomic" => Some(Self::Atomic),
            "privileged" => Some(Self::Privileged),
            _ => None,
        }
    }

    /// The minimum machine class of one frozen platform effect atom. The map is
    /// total over the twelve atoms `.design/build/kernel-primitives.md`
    /// §"Sealed authority and platform effects" freezes, and each row is the
    /// class table in `.design/build/frozen-primitive-registry.md`
    /// §"Source-derived minimum machine class". No atom reaches `sequential`.
    fn of_platform_atom(domain: thermite_syntax::PlatformDomain) -> Self {
        use thermite_syntax::PlatformDomain;
        match domain {
            PlatformDomain::Boot => Self::Privileged,
            PlatformDomain::Memory => Self::Privileged,
            PlatformDomain::Mmio => Self::Volatile,
            PlatformDomain::Pio => Self::Volatile,
            PlatformDomain::Irq => Self::Privileged,
            PlatformDomain::Cpu => Self::Privileged,
            PlatformDomain::Atomic => Self::Atomic,
            PlatformDomain::Smp => Self::Atomic,
            PlatformDomain::Dma => Self::Atomic,
            PlatformDomain::Clock => Self::Volatile,
            PlatformDomain::Entropy => Self::Volatile,
            PlatformDomain::Power => Self::Privileged,
        }
    }
}

/// The source-derived minimum machine class of a Thermite effect row, with the
/// platform atom that produced it. A row carrying no platform atom reaches
/// `sequential` through the empty maximum, which is the domain the safe v1 and
/// v2 linkages keep.
fn source_machine_class(row: &EffectRow) -> (MachineClass, Option<&'static str>) {
    let EffectRow::Set(effects) = row else {
        return (MachineClass::Sequential, None);
    };
    let mut class = MachineClass::Sequential;
    let mut atom = None;
    for effect in effects {
        if let Effect::Platform(domain) = effect {
            let candidate = MachineClass::of_platform_atom(*domain);
            if atom.is_none() || candidate > class {
                class = candidate;
                atom = Some(domain.surface());
            }
        }
    }
    (class, atom)
}

/// The phrase naming what produced an entry's effective machine class: the
/// source effect atom when the source row reaches that class, and the entry's
/// own `concurrency` string when the declaration raises it.
fn effective_class_basis(
    source_class: MachineClass,
    source_atom: Option<&str>,
    declared_class: MachineClass,
    declared: &str,
) -> String {
    match source_atom {
        Some(atom) if source_class >= declared_class => {
            format!("source effect atom `platform({atom})`")
        }
        _ => format!("declared concurrency `{declared}`"),
    }
}

fn parameter_ownership(ty: &Type, sealed: &BTreeSet<&str>) -> String {
    match ty {
        Type::Ref { mutable: true, .. } => "exclusive_borrow".to_string(),
        Type::Ref { mutable: false, .. } => "shared_borrow".to_string(),
        _ if type_contains_sealed(ty, sealed) => "consume_sealed".to_string(),
        _ => "by_value".to_string(),
    }
}

fn result_ownership(ty: &Type, sealed: &BTreeSet<&str>) -> Result<String, String> {
    match ty {
        Type::Ref { .. } | Type::Slice(_) => {
            Err("frozen primitive registry rejects borrowed boundary return types".to_string())
        }
        _ if type_contains_sealed(ty, sealed) => Ok("mint_sealed".to_string()),
        _ => Ok("by_value".to_string()),
    }
}

fn type_contains_sealed(ty: &Type, sealed: &BTreeSet<&str>) -> bool {
    match ty {
        Type::Named(name) => sealed.contains(name.as_str()),
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Array { elem: inner, .. }
        | Type::Generic { arg: inner, .. }
        | Type::Box(inner)
        | Type::Vec(inner)
        | Type::Option(inner) => type_contains_sealed(inner, sealed),
        Type::Result(ok, err) | Type::Map(ok, err) => {
            type_contains_sealed(ok, sealed) || type_contains_sealed(err, sealed)
        }
        Type::Tuple(items) => items.iter().any(|item| type_contains_sealed(item, sealed)),
        Type::Prim(_) | Type::Unit | Type::String => false,
    }
}

fn require_digest(
    semantic_name: &str,
    field: &str,
    found: &str,
    expected: &str,
) -> Result<(), String> {
    if !valid_sha256(found) || found != expected {
        return Err(format!(
            "primitive entry `{semantic_name}` {field} drift: expected `{expected}`, found `{found}`"
        ));
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_semantic_name(name: &str, version: u64) -> bool {
    version > 0
        && name.ends_with(&format!("@v{version}"))
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'@')
        })
}

fn valid_feature(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn valid_evidence_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_memory_ordering(value: &str) -> bool {
    matches!(
        value,
        "relaxed" | "acquire" | "release" | "acq_rel" | "seq_cst"
    )
}

fn require_sorted_unique(
    label: &str,
    values: &[String],
    valid: impl Fn(&str) -> bool,
) -> Result<(), String> {
    if values.iter().any(|value| !valid(value)) {
        return Err(format!(
            "primitive registry contains an invalid {label} value"
        ));
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "primitive registry {label} must be strictly sorted and unique"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (
        Program,
        VerifiedClosure,
        Vec<DirectVerusSource>,
        serde_json::Value,
    ) {
        let parsed = thermite_syntax::parse(
            "#[boundary(\"platform::identity\")] \
             fn platform_identity(value: u64) -> u64 req true ens result == value fx pure; \
             fn observe(value: u64) -> u64 req true ens result == value fx pure { platform_identity(value) }",
        );
        assert!(parsed.is_clean(), "{:?}", parsed.errors);
        let program = parsed.program;
        let closure = closure::verified_closure(&program, &["observe".to_string()]).unwrap();
        let shell_bytes = b"pub fn identity_impl(value: u64) -> (result: u64) ensures result == value, { value }\n".to_vec();
        let shell_plan = super::super::composition::analyze_shell(
            "platform_shell",
            "evidence/direct-verus/00-platform_shell.rs",
            &shell_bytes,
        )
        .unwrap();
        let shell = DirectVerusSource {
            plan: shell_plan,
            bytes: shell_bytes,
        };
        let function = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.name == "platform_identity" => Some(function),
                _ => None,
            })
            .unwrap();
        let registry = serde_json::json!({
            "schema": REGISTRY_SCHEMA_V1,
            "target": {
                "target_triple": "synthetic64-unknown-none",
                "target_features": []
            },
            "entries": [{
                "semantic_name": "synthetic::identity@v1",
                "version": 1,
                "required": true,
                "thermite_name": "platform_identity",
                "boundary_target": "platform::identity",
                "signature": function_signature(function).unwrap(),
                "contract_sha256": semantic_contract_sha256(function),
                "effects_sha256": sha256(format!("{:#?}", function.contract.fx).as_bytes()),
                "effects": ["pure"],
                "ownership": { "parameters": ["by_value"], "result": "by_value" },
                "implementation": {
                    "shell_module": "platform_shell",
                    "item": "identity_impl",
                    "source_sha256": shell.plan.sha256,
                    "abi": "Rust",
                    "symbol": "platform_shell::identity_impl",
                    "alignment": 8
                },
                "model": "thermite_contract",
                "refinement": "same_crate_verus_checked_wrapper",
                "proof_obligations": [
                    "contract_refinement",
                    "exact_implementation_call",
                    "whole_crate_no_cheating"
                ],
                "concurrency": "sequential",
                "memory_orderings": [],
                "failure": "total"
            }]
        });
        (program, closure, vec![shell], registry)
    }

    fn plan(value: &serde_json::Value) -> Result<PrimitiveRegistrySource, String> {
        let (program, closure, shells, _) = fixture();
        load_from_evidence(
            serde_json::to_vec_pretty(value).unwrap(),
            &program,
            &closure,
            &shells,
            "synthetic64-unknown-none",
        )
    }

    fn machine_fixture() -> (
        Program,
        VerifiedClosure,
        Vec<DirectVerusSource>,
        serde_json::Value,
    ) {
        let parsed = thermite_syntax::parse(
            "#[boundary(\"synthetic::atomic::u64::seq_cst_roundtrip\")] \
             fn machine_atomic_roundtrip(value: u64) -> u64 req true ens result == value fx platform(atomic); \
             fn observe(value: u64) -> u64 req true ens result == value fx platform(atomic) { machine_atomic_roundtrip(value) }",
        );
        assert!(parsed.is_clean(), "{:?}", parsed.errors);
        let program = parsed.program;
        let closure = closure::verified_closure(&program, &["observe".to_string()]).unwrap();
        let shell_bytes = b"pub fn atomic_roundtrip_impl(value: u64) -> (result: u64)\n    ensures result == value,\n{\n    let (atomic, Tracked(permission)) = vstd::atomic::PAtomicU64::new(value);\n    atomic.load(Tracked(&permission))\n}\n".to_vec();
        let shell_plan = super::super::composition::analyze_shell(
            "machine_atomic_impl",
            "evidence/direct-verus/00-machine_atomic_impl.rs",
            &shell_bytes,
        )
        .unwrap();
        let shell = DirectVerusSource {
            plan: shell_plan,
            bytes: shell_bytes,
        };
        let function = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.name == "machine_atomic_roundtrip" => Some(function),
                _ => None,
            })
            .unwrap();
        let registry = serde_json::json!({
            "schema": REGISTRY_SCHEMA_V3,
            "target": {
                "target_triple": "synthetic64-unknown-none",
                "target_features": []
            },
            "entries": [{
                "semantic_name": "synthetic::atomic.u64.seq_cst_roundtrip@v1",
                "version": 1,
                "required": true,
                "thermite_name": "machine_atomic_roundtrip",
                "boundary_target": "synthetic::atomic::u64::seq_cst_roundtrip",
                "signature": function_signature(function).unwrap(),
                "contract_sha256": semantic_contract_sha256(function),
                "effects_sha256": sha256(format!("{:#?}", function.contract.fx).as_bytes()),
                "effects": ["platform(atomic)"],
                "ownership": { "parameters": ["by_value"], "result": "by_value" },
                "implementation": {
                    "shell_module": "machine_atomic_impl",
                    "item": "atomic_roundtrip_impl",
                    "source_sha256": shell.plan.sha256,
                    "linkage": "separate_verus_machine_crate",
                    "abi": "Rust",
                    "symbol": "machine_atomic_impl::atomic_roundtrip_impl",
                    "alignment": 8
                },
                "model": "pinned_vstd_atomic_permission",
                "refinement": "separate_crate_verus_machine_import",
                "proof_obligations": [
                    "atomic_permission_refinement",
                    "contract_refinement",
                    "exact_implementation_call",
                    "exported_verus_interface",
                    "imported_call_refinement",
                    "memory_ordering_refinement",
                    "pinned_machine_model",
                    "separate_object_identity",
                    "separate_source_identity",
                    "whole_crate_no_cheating"
                ],
                "concurrency": "atomic",
                "memory_orderings": ["seq_cst"],
                "failure": "total",
                "machine": {
                    "family": "atomic",
                    "operation": "p_atomic_u64_seq_cst_roundtrip",
                    "residual_assumptions": [
                        "hardware_atomic_semantics",
                        "pinned_vstd_external_body",
                        "rust_core_atomic_codegen"
                    ]
                }
            }]
        });
        (program, closure, vec![shell], registry)
    }

    fn plan_machine(value: &serde_json::Value) -> Result<PrimitiveRegistrySource, String> {
        let (program, closure, shells, _) = machine_fixture();
        load_from_evidence(
            serde_json::to_vec_pretty(value).unwrap(),
            &program,
            &closure,
            &shells,
            "synthetic64-unknown-none",
        )
    }

    #[test]
    fn semantic_names_and_digests_are_fail_closed() {
        assert!(valid_semantic_name("kernel::clock.read@v1", 1));
        assert!(!valid_semantic_name("kernel/read@v1", 1));
        assert!(!valid_semantic_name("kernel::clock.read@v2", 1));
        assert!(valid_sha256(&"a".repeat(64)));
        assert!(!valid_sha256(&"A".repeat(64)));
    }

    #[test]
    fn canonical_lists_reject_duplicates_and_bad_values() {
        assert!(require_sorted_unique(
            "proof obligations",
            &["a".to_string(), "b".to_string()],
            valid_evidence_name,
        )
        .is_ok());
        assert!(require_sorted_unique(
            "proof obligations",
            &["b".to_string(), "a".to_string()],
            valid_evidence_name,
        )
        .is_err());
    }

    #[test]
    fn exact_registry_closes_the_reachable_boundary_once() {
        let (_, _, _, mut registry) = fixture();
        registry["target"]["target_features"] = serde_json::json!(["sse2"]);
        let planned = plan(&registry).unwrap();
        assert_eq!(planned.bindings.len(), 1);
        assert_eq!(planned.bindings[0].source_name, "platform_identity");
        assert_eq!(
            planned.bindings[0].call_target,
            "platform_shell::identity_impl"
        );
        assert!(planned.plan.entries[0].reachable);
        assert_eq!(planned.plan.entries[0].proof_obligations.len(), 3);
        assert_eq!(planned.plan.target.target_features, ["sse2"]);
    }

    #[test]
    fn registry_v2_separates_source_and_requires_the_full_import_proof_set() {
        let (_, _, _, mut registry) = fixture();
        registry["schema"] = serde_json::json!(REGISTRY_SCHEMA_V2);
        registry["entries"][0]["implementation"]["linkage"] =
            serde_json::json!("separate_verus_crate");
        registry["entries"][0]["refinement"] = serde_json::json!("separate_crate_verus_import");
        registry["entries"][0]["proof_obligations"] = serde_json::json!([
            "contract_refinement",
            "exact_implementation_call",
            "exported_verus_interface",
            "imported_call_refinement",
            "separate_object_identity",
            "separate_source_identity",
            "whole_crate_no_cheating"
        ]);
        let planned = plan(&registry).unwrap();
        assert_eq!(
            planned.separate_crates,
            BTreeSet::from(["platform_shell".to_string()])
        );
        assert_eq!(
            planned.plan.entries[0].implementation_linkage,
            "separate_verus_crate"
        );
        assert_eq!(planned.plan.entries[0].proof_obligations.len(), 7);

        let mut missing = registry.clone();
        missing["entries"][0]["proof_obligations"] = serde_json::json!([
            "contract_refinement",
            "exact_implementation_call",
            "exported_verus_interface",
            "imported_call_refinement",
            "separate_source_identity",
            "whole_crate_no_cheating"
        ]);
        assert!(plan(&missing)
            .unwrap_err()
            .contains("separate_object_identity"));

        let mut v2_without_linkage = registry.clone();
        v2_without_linkage["entries"][0]["implementation"]
            .as_object_mut()
            .unwrap()
            .remove("linkage");
        assert!(plan(&v2_without_linkage)
            .unwrap_err()
            .contains("must declare implementation.linkage"));

        let mut v1_with_linkage = registry;
        v1_with_linkage["schema"] = serde_json::json!(REGISTRY_SCHEMA_V1);
        assert!(plan(&v1_with_linkage)
            .unwrap_err()
            .contains("cannot declare implementation.linkage in registry v1"));
    }

    #[test]
    fn registry_v3_admits_only_the_explicitly_residual_atomic_pilot() {
        let (_, _, _, registry) = machine_fixture();
        let planned = plan_machine(&registry).unwrap();
        assert_eq!(
            planned.machine_crates,
            BTreeSet::from(["machine_atomic_impl".to_string()])
        );
        assert_eq!(
            planned.machine_boundaries,
            BTreeSet::from(["machine_atomic_roundtrip".to_string()])
        );
        assert_eq!(
            planned.plan.entries[0].residual_assumptions,
            [
                "hardware_atomic_semantics",
                "pinned_vstd_external_body",
                "rust_core_atomic_codegen"
            ]
        );

        let mut missing_residual = registry.clone();
        missing_residual["entries"][0]["machine"]["residual_assumptions"] =
            serde_json::json!(["hardware_atomic_semantics", "pinned_vstd_external_body"]);
        assert!(plan_machine(&missing_residual)
            .unwrap_err()
            .contains("exact residual assumptions"));

        let mut weaker_order = registry.clone();
        weaker_order["entries"][0]["memory_orderings"] = serde_json::json!(["acquire"]);
        assert!(plan_machine(&weaker_order)
            .unwrap_err()
            .contains("atomic u64 SeqCst roundtrip"));

        let mut safe_schema = registry;
        safe_schema["schema"] = serde_json::json!(REGISTRY_SCHEMA_V2);
        assert!(plan_machine(&safe_schema)
            .unwrap_err()
            .contains("requires frozen primitive registry v3"));
    }

    #[test]
    fn target_features_are_canonical_bare_sorted_names() {
        let (_, _, _, registry) = fixture();

        let mut changed = registry.clone();
        changed["target"]["target_features"] = serde_json::json!(["sse2", "sse2"]);
        assert!(plan(&changed)
            .unwrap_err()
            .contains("strictly sorted and unique"));

        let mut changed = registry.clone();
        changed["target"]["target_features"] = serde_json::json!(["+sse2"]);
        assert!(plan(&changed)
            .unwrap_err()
            .contains("invalid target features value"));

        let mut changed = registry;
        changed["target"]["target_features"] = serde_json::json!(["zeta", "alpha"]);
        assert!(plan(&changed)
            .unwrap_err()
            .contains("strictly sorted and unique"));
    }

    #[test]
    fn registry_rejects_schema_source_contract_effect_abi_and_ownership_drift() {
        let (_, _, _, registry) = fixture();

        let mut changed = registry.clone();
        changed["unknown"] = serde_json::json!(true);
        assert!(plan(&changed).unwrap_err().contains("unknown field"));

        let mut changed = registry.clone();
        changed["target"]["target_triple"] = serde_json::json!("wrong-target");
        assert!(plan(&changed).unwrap_err().contains("target triple"));

        let mut changed = registry.clone();
        changed["entries"][0]["signature"] = serde_json::json!("fn drift()->u64");
        assert!(plan(&changed).unwrap_err().contains("signature drift"));

        let mut changed = registry.clone();
        changed["entries"][0]["contract_sha256"] = serde_json::json!("0".repeat(64));
        assert!(plan(&changed)
            .unwrap_err()
            .contains("contract_sha256 drift"));

        let mut changed = registry.clone();
        changed["entries"][0]["effects"] = serde_json::json!(["platform(cpu)"]);
        assert!(plan(&changed).unwrap_err().contains("effect-row drift"));

        let mut changed = registry.clone();
        changed["entries"][0]["implementation"]["source_sha256"] =
            serde_json::json!("0".repeat(64));
        assert!(plan(&changed)
            .unwrap_err()
            .contains("implementation.source_sha256 drift"));

        let mut changed = registry.clone();
        changed["entries"][0]["implementation"]["abi"] = serde_json::json!("C");
        assert!(plan(&changed).unwrap_err().contains("supports only `Rust`"));

        let mut changed = registry.clone();
        changed["entries"][0]["ownership"]["parameters"] = serde_json::json!(["shared_borrow"]);
        assert!(plan(&changed).unwrap_err().contains("ownership drift"));

        let mut changed = registry.clone();
        changed["entries"][0]["proof_obligations"] =
            serde_json::json!(["contract_refinement", "exact_implementation_call"]);
        assert!(plan(&changed)
            .unwrap_err()
            .contains("whole_crate_no_cheating"));
    }

    #[test]
    fn safe_registry_linkages_refuse_machine_concurrency_assurance_claims() {
        let (_, _, _, registry) = fixture();
        for concurrency in ["atomic", "volatile", "privileged"] {
            let mut changed = registry.clone();
            changed["entries"][0]["concurrency"] = serde_json::json!(concurrency);
            changed["entries"][0]["memory_orderings"] = if concurrency == "atomic" {
                serde_json::json!(["acquire", "relaxed"])
            } else {
                serde_json::json!([])
            };
            let error = plan(&changed).expect_err("v1 must not overclaim machine refinement");
            assert!(error.contains("can directly refine only sequential safe-Rust"));
            assert!(error.contains("registry v3 machine evidence"));
        }
    }
}
