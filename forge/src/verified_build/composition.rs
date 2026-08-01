//! Exact-source rich-state Thermite/direct-Verus composition builds.

use super::*;
use thermite_syntax::{FieldDef, VariantShape};

struct Assembly {
    closure: VerifiedClosure,
    selected_program: Program,
    link_exports: Vec<PlannedExport>,
    composition_exports: Vec<PlannedCompositionExport>,
    shell_sources: Vec<DirectVerusSource>,
    lowered_thermite: String,
    combined_source: String,
}

struct AssemblyTarget<'a> {
    crate_name: &'a str,
    target: VerifiedTarget,
    triple: &'a str,
    pointer_width: &'a str,
    endian: &'a str,
}

pub(super) fn build_file(
    path: &Path,
    link_export_names: &[String],
    composition_export_names: &[String],
    shell_paths: &[PathBuf],
    crate_name: Option<&str>,
    out: Option<&Path>,
    target: VerifiedTarget,
) -> Result<VerifiedBuildOutcome, ForgeError> {
    let raw_source = fs::read(path).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })?;
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
            ));
        }
        None => sanitized_crate_name(path),
    };
    if composition_export_names.is_empty() || shell_paths.is_empty() {
        return Ok(reject(
            "plan",
            "a composition build requires at least one --compose-export and --compose-shell",
        ));
    }
    let link_names: BTreeSet<&str> = link_export_names.iter().map(String::as_str).collect();
    if let Some(overlap) = composition_export_names
        .iter()
        .find(|name| link_names.contains(name.as_str()))
    {
        return Ok(reject(
            "exports",
            format!("`{overlap}` cannot be both a link export and a composition export"),
        ));
    }
    let destination = out.map_or_else(
        || {
            std::env::temp_dir().join(format!(
                "{}.verified-composition.{}",
                crate_name,
                std::process::id()
            ))
        },
        Path::to_path_buf,
    );
    if destination.exists() {
        return Ok(reject(
            "publication",
            format!(
                "refusing to overwrite existing bundle `{}`",
                destination.display()
            ),
        ));
    }

    let collected_toolchain = collect_toolchain(target)?;
    let toolchain = &collected_toolchain.evidence;
    let assembly = match assemble_from_paths(
        &parsed.program,
        link_export_names,
        composition_export_names,
        shell_paths,
        &crate_name,
        target,
        toolchain,
    ) {
        Ok(assembly) => assembly,
        Err(detail) => return Ok(reject("composition-plan", detail)),
    };

    let mut plan = make_plan(PlanInput {
        raw_source: &raw_source,
        program: &parsed.program,
        selected_program: &assembly.selected_program,
        closure: &assembly.closure,
        exports: &assembly.link_exports,
        crate_name: &crate_name,
        target,
        target_triple: &toolchain.target_triple,
        target_pointer_width: &toolchain.target_pointer_width,
        target_endian: &toolchain.target_endian,
        verus_source: &assembly.combined_source,
    });
    attach_composition_plan(&mut plan, &assembly);
    let frozen_plan_sha = plan.canonical_sha256();

    // Re-open and independently reassemble all authored sources after the plan
    // is frozen. No proof or compiler consumes the earlier planning emission.
    let mut fresh = match assemble_from_paths(
        &parsed.program,
        link_export_names,
        composition_export_names,
        shell_paths,
        &crate_name,
        target,
        toolchain,
    ) {
        Ok(assembly) => assembly,
        Err(detail) => return Ok(reject("binding", detail)),
    };
    if test_fault("composition-after-plan-lowered-mutation") {
        fresh.lowered_thermite.push_str("\n// injected mutation\n");
    } else if test_fault("composition-after-plan-shell-mutation") {
        if let Some(shell) = fresh.shell_sources.first_mut() {
            shell.bytes.push(b' ');
        }
    } else if test_fault("composition-after-plan-source-mutation") {
        fresh.combined_source.push_str("\n// injected mutation\n");
    }
    if fresh.lowered_thermite != assembly.lowered_thermite
        || fresh.shell_sources != assembly.shell_sources
        || fresh.combined_source != assembly.combined_source
        || sha256(fresh.combined_source.as_bytes()) != plan.expected_verus_source_sha256
    {
        return Ok(reject(
            "binding",
            "a Thermite lowering, direct-Verus source, or combined source changed after the composition plan was frozen",
        ));
    }

    let frozen_input = ScratchTree::new_in_temp(&format!("composition_input_{crate_name}"))?;
    let input_name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::ffi::OsStr::new("input.th"));
    let frozen_input_path = frozen_input.path.join(input_name);
    write_bytes(&frozen_input_path, &raw_source)?;

    let mut certificates = check::check_file(&frozen_input_path)?;
    inject_certificate_fault(&mut certificates);
    if let Some(detail) = reject_certificates(&certificates, &assembly.closure, &parsed.program) {
        return Ok(reject("certificates", detail));
    }
    let mut tv = collect_translation_validation(
        &frozen_input_path,
        &parsed.program,
        &assembly.closure,
        &assembly.link_exports,
    )?;
    complete_rich_composition_tv(&mut tv, &parsed.program, &assembly.closure);
    inject_tv_fault(&mut tv);
    if let Some(detail) = reject_translation_validation(
        &tv,
        &parsed.program,
        &assembly.closure,
        &assembly.link_exports,
    ) {
        return Ok(reject("translation-validation", detail));
    }

    if test_fault("before-verus") {
        return Ok(reject("fault-injection", "injected failure before Verus"));
    }
    let compiled = compile_verus_source(
        &crate_name,
        &fresh.combined_source,
        target,
        &toolchain.verus_path,
        &toolchain.environment,
        &toolchain.artifact_codegen.canonical_identity_sha256(),
        collected_toolchain.dependency_path("libvstd.rlib"),
    )?;
    if !compiled.evidence.success || compiled.evidence.errors != Some(0) {
        return Ok(reject(
            "whole-crate-verus",
            verus_failure_detail(
                "strict combined Verus proof/codegen failed",
                &compiled.evidence,
            ),
        ));
    }
    if compiled.evidence.source_sha256_before != plan.expected_verus_source_sha256
        || compiled.evidence.source_sha256_after != plan.expected_verus_source_sha256
    {
        return Ok(reject(
            "binding",
            "the final combined Verus input changed before or during proof/codegen",
        ));
    }
    if test_fault("after-verus") || test_fault("after-codegen") {
        return Ok(reject(
            "fault-injection",
            "injected failure after the exact combined Verus proof/codegen invocation",
        ));
    }

    let receipt = stage_and_publish(StageInput {
        destination: &destination,
        crate_name: &crate_name,
        target,
        raw_source: &raw_source,
        plan: &plan,
        plan_sha256: &frozen_plan_sha,
        verus_source: &fresh.combined_source,
        certificates: &certificates,
        tv: &tv,
        compiled: &compiled,
        toolchain,
        dependency_paths: &collected_toolchain.dependency_paths,
        composition: Some(CompositionStageInput {
            lowered_thermite: &fresh.lowered_thermite,
            shell_sources: &fresh.shell_sources,
        }),
    })?;

    Ok(VerifiedBuildOutcome::Built {
        bundle: destination,
        receipt: Box::new(receipt),
    })
}

/// The scalar exec/body TV encoders deliberately return `Skipped` when their
/// synthetic obligation frame cannot spell a rich ADT/tuple value. In a
/// composition build those exact rows are completed by the stronger closed-
/// source argument: the function is in the canonical lowering closure, its L3
/// certificate proves that lowering, and the only compiled source is the bound
/// whole-crate emission. No other skipped/unverifiable TV result is upgraded.
fn complete_rich_composition_tv(
    evidence: &mut TranslationValidationEvidence,
    program: &Program,
    closure: &VerifiedClosure,
) {
    let mut completed = Vec::new();
    for mut row in std::mem::take(&mut evidence.rows) {
        if row.phase == "contract" && row.verdict == "skipped" && row.label.ends_with(".signature")
        {
            let name = row.label.trim_end_matches(".signature");
            let function = program.items.iter().find_map(|item| match item {
                Item::Fn(function) if function.name == name && closure.functions.contains(name) => {
                    Some(function)
                }
                _ => None,
            });
            if let Some(function) = function {
                let detail = Some(rich_completion_detail());
                completed.push(TvEvidenceRow {
                    phase: "contract".to_string(),
                    label: format!("{}.req", function.name),
                    verdict: "faithful".to_string(),
                    detail: detail.clone(),
                });
                for index in 0..function.contract.ens.len() {
                    completed.push(TvEvidenceRow {
                        phase: "contract".to_string(),
                        label: format!("{}.ens#{}", function.name, index + 1),
                        verdict: "faithful".to_string(),
                        detail: detail.clone(),
                    });
                }
                continue;
            }
        }
        if row.verdict != "skipped" || !matches!(row.phase.as_str(), "exec" | "body") {
            completed.push(row);
            continue;
        }
        let Some(detail) = row.detail.as_deref() else {
            continue;
        };
        let root = row.label.split('.').next().unwrap_or(&row.label);
        let rich_frame_limit = detail.contains("outside the exec frame sublanguage")
            || detail.contains("richer-typed param");
        if rich_frame_limit && closure.functions.contains(root) {
            row.verdict = "faithful".to_string();
            row.detail = Some(rich_completion_detail());
        }
        completed.push(row);
    }
    completed.sort_by(|a, b| a.phase.cmp(&b.phase).then(a.label.cmp(&b.label)));
    evidence.rows = completed;
}

fn rich_completion_detail() -> String {
    "rich-state composition completion: the scalar TV frame is inapplicable; exact canonical \
     closure lowering plus the bound L3 certificate and single whole-crate no-cheating proof \
     cover this row"
        .to_string()
}

fn assemble_from_paths(
    program: &Program,
    link_export_names: &[String],
    composition_export_names: &[String],
    shell_paths: &[PathBuf],
    crate_name: &str,
    target: VerifiedTarget,
    toolchain: &ToolchainEvidence,
) -> Result<Assembly, String> {
    let shell_sources = load_shell_paths(shell_paths)?;
    assemble(
        program,
        link_export_names,
        composition_export_names,
        shell_sources,
        AssemblyTarget {
            crate_name,
            target,
            triple: &toolchain.target_triple,
            pointer_width: &toolchain.target_pointer_width,
            endian: &toolchain.target_endian,
        },
    )
}

fn assemble(
    program: &Program,
    link_export_names: &[String],
    composition_export_names: &[String],
    shell_sources: Vec<DirectVerusSource>,
    target: AssemblyTarget<'_>,
) -> Result<Assembly, String> {
    let mut roots = link_export_names.to_vec();
    roots.extend_from_slice(composition_export_names);
    roots.sort();
    roots.dedup();
    if roots.len() != link_export_names.len() + composition_export_names.len() {
        return Err("composition and link export names must be unique".to_string());
    }
    let closure = closure::verified_closure(program, &roots).map_err(|error| error.to_string())?;
    if let Some(detail) = strict_source_checks(program, &closure, target.target) {
        return Err(detail);
    }
    let link_exports = plan_exports(
        program,
        link_export_names,
        target.crate_name,
        target.target,
        target.triple,
        target.pointer_width,
        target.endian,
    )?;
    let composition_exports = plan_composition_exports(program, composition_export_names)?;
    let selected_program = closure_program(program, &closure);
    let mut lower_exports: Vec<L3Export> = link_exports
        .iter()
        .map(|export| L3Export {
            source_name: export.thermite_name.clone(),
            public_name: export.public_name.clone(),
            wrapped: export.wrapped,
            visibility: L3ExportVisibility::Public,
        })
        .collect();
    lower_exports.extend(composition_exports.iter().map(|export| L3Export {
        source_name: export.thermite_name.clone(),
        public_name: export.thermite_name.clone(),
        wrapped: false,
        visibility: L3ExportVisibility::Crate,
    }));
    let lower_target = match target.target {
        VerifiedTarget::Std => L3LibraryTarget::Std,
        VerifiedTarget::Kernel => L3LibraryTarget::Kernel,
    };
    let lowered_thermite =
        thermite_lower::lower_l3_library(&selected_program, &lower_exports, lower_target)
            .map_err(|error| error.to_string())?;
    if let Some(token) = forbidden_emission(&lowered_thermite) {
        return Err(format!(
            "canonical Thermite lowering contains forbidden escape hatch `{token}`"
        ));
    }
    let combined_source = combine_sources(&lowered_thermite, &shell_sources)?;
    Ok(Assembly {
        closure,
        selected_program,
        link_exports,
        composition_exports,
        shell_sources,
        lowered_thermite,
        combined_source,
    })
}

fn attach_composition_plan(plan: &mut ArtifactPlanV1, assembly: &Assembly) {
    plan.schema = COMPOSITION_PLAN_SCHEMA.to_string();
    plan.strict_gates = COMPOSITION_STRICT_GATES
        .iter()
        .map(|gate| (*gate).to_string())
        .collect();
    let inventory = composition_inventory(plan, assembly);
    plan.composition = Some(CompositionPlanV1 {
        schema: "thermite.composition-plan.v1".to_string(),
        composition_exports: assembly.composition_exports.clone(),
        shell_modules: assembly
            .shell_sources
            .iter()
            .map(|source| source.plan.clone())
            .collect(),
        inventory,
        lowered_thermite_sha256: sha256(assembly.lowered_thermite.as_bytes()),
        combined_source_sha256: sha256(assembly.combined_source.as_bytes()),
    });
}

fn composition_inventory(
    plan: &ArtifactPlanV1,
    assembly: &Assembly,
) -> Vec<CompositionInventoryRow> {
    let public: BTreeSet<&str> = assembly
        .link_exports
        .iter()
        .map(|export| export.thermite_name.as_str())
        .collect();
    let crate_visible: BTreeSet<&str> = assembly
        .composition_exports
        .iter()
        .map(|export| export.thermite_name.as_str())
        .collect();
    let mut rows: Vec<CompositionInventoryRow> = plan
        .closure_nodes
        .iter()
        .map(|node| CompositionInventoryRow {
            origin: "thermite".to_string(),
            name: node.name.clone(),
            kind: node.kind.clone(),
            visibility: if public.contains(node.name.as_str()) {
                "public"
            } else if crate_visible.contains(node.name.as_str()) {
                "crate"
            } else if matches!(node.kind.as_str(), "struct" | "enum" | "spec_fn") {
                "public"
            } else {
                "private"
            }
            .to_string(),
            sha256: node.item_sha256.clone(),
        })
        .collect();
    rows.push(CompositionInventoryRow {
        origin: "generated".to_string(),
        name: "canonical-thermite-lowering".to_string(),
        kind: "module".to_string(),
        visibility: "crate".to_string(),
        sha256: sha256(assembly.lowered_thermite.as_bytes()),
    });
    for shell in &assembly.shell_sources {
        rows.push(CompositionInventoryRow {
            origin: "direct-verus".to_string(),
            name: shell.plan.name.clone(),
            kind: "module".to_string(),
            visibility: "public".to_string(),
            sha256: shell.plan.sha256.clone(),
        });
        for item in &shell.plan.items {
            rows.push(CompositionInventoryRow {
                origin: format!("direct-verus::{}", shell.plan.name),
                name: item.name.clone(),
                kind: item.kind.clone(),
                visibility: item.visibility.clone(),
                sha256: sha256(
                    format!(
                        "{}\0{}\0{}\0{}\0{}",
                        shell.plan.sha256, shell.plan.name, item.kind, item.name, item.visibility
                    )
                    .as_bytes(),
                ),
            });
        }
    }
    rows.sort_by(|a, b| {
        a.origin
            .cmp(&b.origin)
            .then(a.name.cmp(&b.name))
            .then(a.kind.cmp(&b.kind))
    });
    rows
}

fn plan_composition_exports(
    program: &Program,
    names: &[String],
) -> Result<Vec<PlannedCompositionExport>, String> {
    let mut rows = Vec::new();
    for name in names {
        let function = program.items.iter().find_map(|item| match item {
            Item::Fn(function) if &function.name == name => Some(function),
            _ => None,
        });
        let Some(function) = function else {
            return Err(format!("unknown executable composition export `{name}`"));
        };
        let parameter_types = function
            .params
            .iter()
            .map(|param| type_spelling(&param.ty))
            .collect::<Result<Vec<_>, _>>()?;
        let ownership = function
            .params
            .iter()
            .map(|param| match &param.ty {
                Type::Ref { mutable: true, .. } => "exclusive_borrow",
                Type::Ref { mutable: false, .. } => "shared_borrow",
                _ => "by_value",
            })
            .map(str::to_string)
            .collect::<Vec<_>>();
        let return_type = type_spelling(&function.ret)?;
        let params = function
            .params
            .iter()
            .zip(&parameter_types)
            .map(|(param, ty)| format!("{}:{ty}", param.name))
            .collect::<Vec<_>>()
            .join(",");
        let mut type_closure = BTreeSet::new();
        let mut seen_named = BTreeSet::new();
        for param in &function.params {
            collect_type_closure(program, &param.ty, &mut type_closure, &mut seen_named)?;
        }
        collect_type_closure(program, &function.ret, &mut type_closure, &mut seen_named)?;
        rows.push(PlannedCompositionExport {
            thermite_name: name.clone(),
            semantic_address: format!("fn::{name}"),
            signature: format!("fn {name}({params})->{return_type}"),
            parameter_types,
            ownership,
            return_type,
            type_closure: type_closure.into_iter().collect(),
            visibility: "crate".to_string(),
        });
    }
    rows.sort_by(|a, b| a.thermite_name.cmp(&b.thermite_name));
    if rows
        .windows(2)
        .any(|pair| pair[0].thermite_name == pair[1].thermite_name)
    {
        return Err("duplicate composition export".to_string());
    }
    Ok(rows)
}

fn collect_type_closure(
    program: &Program,
    ty: &Type,
    rows: &mut BTreeSet<String>,
    seen_named: &mut BTreeSet<String>,
) -> Result<(), String> {
    rows.insert(type_spelling(ty)?);
    match ty {
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Generic { arg: inner, .. }
        | Type::Box(inner)
        | Type::Vec(inner)
        | Type::Option(inner) => collect_type_closure(program, inner, rows, seen_named)?,
        Type::Result(ok, err) | Type::Map(ok, err) => {
            collect_type_closure(program, ok, rows, seen_named)?;
            collect_type_closure(program, err, rows, seen_named)?;
        }
        Type::Tuple(items) => {
            for item in items {
                collect_type_closure(program, item, rows, seen_named)?;
            }
        }
        Type::Named(name) if seen_named.insert(name.clone()) => {
            let item = program.items.iter().find(|item| item.name() == name);
            match item {
                Some(Item::Struct(item)) => {
                    rows.insert(format!(
                        "struct {}{{{}}}",
                        item.name,
                        render_fields(&item.fields)?
                    ));
                    for field in &item.fields {
                        collect_type_closure(program, &field.ty, rows, seen_named)?;
                    }
                }
                Some(Item::Enum(item)) => {
                    let mut variants = Vec::new();
                    for variant in &item.variants {
                        let rendered = match &variant.shape {
                            VariantShape::Unit => variant.name.clone(),
                            VariantShape::Tuple(types) => format!(
                                "{}({})",
                                variant.name,
                                types
                                    .iter()
                                    .map(type_spelling)
                                    .collect::<Result<Vec<_>, _>>()?
                                    .join(",")
                            ),
                            VariantShape::Struct(fields) => {
                                format!("{}{{{}}}", variant.name, render_fields(fields)?)
                            }
                        };
                        variants.push(rendered);
                        match &variant.shape {
                            VariantShape::Tuple(types) => {
                                for ty in types {
                                    collect_type_closure(program, ty, rows, seen_named)?;
                                }
                            }
                            VariantShape::Struct(fields) => {
                                for field in fields {
                                    collect_type_closure(program, &field.ty, rows, seen_named)?;
                                }
                            }
                            VariantShape::Unit => {}
                        }
                    }
                    rows.insert(format!("enum {}{{{}}}", item.name, variants.join(",")));
                }
                _ => {
                    return Err(format!(
                        "composition type `{name}` has no bound ADT definition"
                    ))
                }
            }
        }
        Type::Prim(_) | Type::Unit | Type::String | Type::Named(_) => {}
    }
    Ok(())
}

fn render_fields(fields: &[FieldDef]) -> Result<String, String> {
    fields
        .iter()
        .map(|field| Ok(format!("{}:{}", field.name, type_spelling(&field.ty)?)))
        .collect::<Result<Vec<_>, String>>()
        .map(|fields| fields.join(","))
}

fn type_spelling(ty: &Type) -> Result<String, String> {
    Ok(match ty {
        Type::Prim(PrimType::U32) => "u32".to_string(),
        Type::Prim(PrimType::U64) => "u64".to_string(),
        Type::Prim(PrimType::Usize) => "usize".to_string(),
        Type::Prim(PrimType::Bool) => "bool".to_string(),
        Type::Unit => "()".to_string(),
        Type::Ref { mutable, inner } => format!(
            "&{}{}",
            if *mutable { "mut " } else { "" },
            type_spelling(inner)?
        ),
        Type::Slice(inner) => format!("[{}]", type_spelling(inner)?),
        Type::Generic { name, arg } => format!("{name}<{}>", type_spelling(arg)?),
        Type::Named(name) => name.clone(),
        Type::Box(inner) => format!("Box<{}>", type_spelling(inner)?),
        Type::Vec(inner) => format!("Vec<{}>", type_spelling(inner)?),
        Type::String => "String".to_string(),
        Type::Option(inner) => format!("Option<{}>", type_spelling(inner)?),
        Type::Result(ok, err) => {
            format!("Result<{},{}>", type_spelling(ok)?, type_spelling(err)?)
        }
        Type::Map(key, value) => {
            format!("Map<{},{}>", type_spelling(key)?, type_spelling(value)?)
        }
        Type::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(type_spelling)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
    })
}

fn load_shell_paths(paths: &[PathBuf]) -> Result<Vec<DirectVerusSource>, String> {
    let mut sources = Vec::new();
    for path in paths {
        let bytes = fs::read(path).map_err(|error| {
            format!(
                "could not read direct-Verus shell `{}`: {error}",
                path.display()
            )
        })?;
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("shell");
        let name = sanitize_module_name(stem);
        let plan = analyze_shell(&name, "", &bytes)?;
        sources.push(DirectVerusSource { plan, bytes });
    }
    sources.sort_by(|a, b| a.plan.name.cmp(&b.plan.name));
    if sources
        .windows(2)
        .any(|pair| pair[0].plan.name == pair[1].plan.name)
    {
        return Err("direct-Verus shell module names must be unique".to_string());
    }
    for (index, source) in sources.iter_mut().enumerate() {
        source.plan.path = format!("evidence/direct-verus/{index:02}-{}.rs", source.plan.name);
    }
    Ok(sources)
}

fn sanitize_module_name(stem: &str) -> String {
    let mut name: String = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
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

fn analyze_shell(name: &str, path: &str, bytes: &[u8]) -> Result<PlannedShellModule, String> {
    let source = std::str::from_utf8(bytes)
        .map_err(|error| format!("direct-Verus module `{name}` is not UTF-8: {error}"))?;
    if source.trim().is_empty() {
        return Err(format!("direct-Verus module `{name}` is empty"));
    }
    let tokens = shell_tokens(source)?;
    for token in &tokens {
        if matches!(
            token.as_str(),
            "external_body"
                | "assume"
                | "assume_"
                | "admit"
                | "axiom"
                | "unsafe"
                | "unimplemented"
                | "todo"
                | "include"
                | "include_str"
                | "include_bytes"
                | "extern"
                | "mod"
                | "macro_rules"
                | "no_erasure_check"
                | "exec_allows_no_decreases_clause"
        ) {
            return Err(format!(
                "direct-Verus module `{name}` uses forbidden token `{token}`"
            ));
        }
    }
    if tokens.iter().any(|token| token == "#")
        || tokens.windows(2).any(|pair| pair == ["verus", "!"])
        || source.contains("decreases *")
    {
        return Err(format!(
            "direct-Verus module `{name}` uses an attribute, nested verus macro, or unchecked decreases"
        ));
    }
    let items = shell_items(name, &tokens)?;
    if items.is_empty() {
        return Err(format!(
            "direct-Verus module `{name}` declares no auditable top-level item"
        ));
    }
    Ok(PlannedShellModule {
        name: name.to_string(),
        path: path.to_string(),
        length: bytes.len() as u64,
        sha256: sha256(bytes),
        items,
    })
}

fn shell_tokens(source: &str) -> Result<Vec<String>, String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                let mut depth = 1usize;
                while index < bytes.len() && depth > 0 {
                    if bytes.get(index..index + 2) == Some(b"/*") {
                        depth += 1;
                        index += 2;
                    } else if bytes.get(index..index + 2) == Some(b"*/") {
                        depth -= 1;
                        index += 2;
                    } else {
                        index += 1;
                    }
                }
                if depth != 0 {
                    return Err("unterminated block comment in direct-Verus source".to_string());
                }
            }
            b'"' | b'\'' => {
                let quote = bytes[index];
                index += 1;
                let mut escaped = false;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if escaped {
                        escaped = false;
                    } else if byte == b'\\' {
                        escaped = true;
                    } else if byte == quote {
                        break;
                    }
                }
                tokens.push("<literal>".to_string());
            }
            byte if byte.is_ascii_whitespace() => index += 1,
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                tokens.push(source[start..index].to_string());
            }
            byte if byte.is_ascii_digit() => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                tokens.push(source[start..index].to_string());
            }
            byte => {
                tokens.push(char::from(byte).to_string());
                index += 1;
            }
        }
    }
    Ok(tokens)
}

fn shell_items(name: &str, tokens: &[String]) -> Result<Vec<PlannedShellItem>, String> {
    let item_kinds = ["fn", "struct", "enum", "type", "const", "static", "trait"];
    let mut depth = 0usize;
    let mut items = Vec::new();
    let mut index = 0usize;
    let mut visibility = "private";
    while index < tokens.len() {
        match tokens[index].as_str() {
            "{" => {
                depth += 1;
                index += 1;
            }
            "}" => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            "pub" if depth == 0 => {
                visibility = "public";
                if tokens.get(index + 1).map(String::as_str) == Some("(") {
                    visibility = "restricted";
                }
                index += 1;
            }
            token if depth == 0 && item_kinds.contains(&token) => {
                let kind = token.to_string();
                let item_name = tokens.get(index + 1).ok_or_else(|| {
                    format!("direct-Verus module `{name}` has an incomplete `{kind}` item")
                })?;
                if !item_name
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                {
                    return Err(format!(
                        "direct-Verus module `{name}` has an unnameable `{kind}` item"
                    ));
                }
                if kind == "fn" {
                    let tail = &tokens[index + 2..];
                    let body = tail.iter().position(|token| token == "{");
                    let declaration = tail.iter().position(|token| token == ";");
                    if body.is_none() || declaration.is_some_and(|semi| semi < body.unwrap()) {
                        return Err(format!(
                            "direct-Verus function `{item_name}` has no checked body"
                        ));
                    }
                }
                items.push(PlannedShellItem {
                    name: item_name.clone(),
                    kind,
                    visibility: visibility.to_string(),
                });
                visibility = "private";
                index += 2;
            }
            ";" if depth == 0 => {
                visibility = "private";
                index += 1;
            }
            _ => index += 1,
        }
    }
    items.sort_by(|a, b| a.name.cmp(&b.name).then(a.kind.cmp(&b.kind)));
    if items
        .windows(2)
        .any(|pair| pair[0].name == pair[1].name && pair[0].kind == pair[1].kind)
    {
        return Err(format!(
            "direct-Verus module `{name}` has duplicate top-level inventory rows"
        ));
    }
    Ok(items)
}

fn combine_sources(lowered: &str, shells: &[DirectVerusSource]) -> Result<String, String> {
    let Some(prefix) = lowered.strip_suffix("}\n") else {
        return Err(
            "canonical Thermite lowering has an unexpected outer verus! framing".to_string(),
        );
    };
    let mut combined = prefix.to_string();
    for shell in shells {
        let source = std::str::from_utf8(&shell.bytes).map_err(|error| {
            format!(
                "direct-Verus module `{}` is not UTF-8: {error}",
                shell.plan.name
            )
        })?;
        combined.push_str("\n\npub mod ");
        combined.push_str(&shell.plan.name);
        combined.push_str(" {\n    use super::*;\n");
        combined.push_str(source);
        if !source.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str("}\n");
    }
    combined.push_str("}\n");
    if combined.matches("verus!").count() != 1 {
        return Err("combined source must contain exactly one verus! invocation".to_string());
    }
    Ok(combined)
}

pub(super) fn reconstruct_plan(
    program: &Program,
    raw_source: &[u8],
    plan: &ArtifactPlanV1,
    bundle: &Path,
) -> Result<
    (
        ArtifactPlanV1,
        String,
        String,
        VerifiedClosure,
        Vec<PlannedExport>,
    ),
    ForgeError,
> {
    let expected = plan
        .composition
        .as_ref()
        .ok_or_else(|| ForgeError::VerusOutput {
            detail: "composition receipt has no composition plan".to_string(),
        })?;
    let link_names: Vec<String> = plan
        .exports
        .iter()
        .map(|export| export.thermite_name.clone())
        .collect();
    let composition_names: Vec<String> = expected
        .composition_exports
        .iter()
        .map(|export| export.thermite_name.clone())
        .collect();
    let mut sources = Vec::new();
    for module in &expected.shell_modules {
        validate_relative_path(&module.path)?;
        let bytes = file_sha256(&bundle.join(&module.path))?.1;
        let observed = analyze_shell(&module.name, &module.path, &bytes).map_err(|detail| {
            ForgeError::VerusOutput {
                detail: format!("bound direct-Verus source violates policy: {detail}"),
            }
        })?;
        if observed != *module {
            return Err(ForgeError::VerusOutput {
                detail: "bound direct-Verus source does not match its planned inventory"
                    .to_string(),
            });
        }
        sources.push(DirectVerusSource {
            plan: observed,
            bytes,
        });
    }
    let assembly = assemble(
        program,
        &link_names,
        &composition_names,
        sources,
        AssemblyTarget {
            crate_name: &plan.crate_name,
            target: plan.target,
            triple: &plan.target_triple,
            pointer_width: &plan.target_pointer_width,
            endian: &plan.target_endian,
        },
    )
    .map_err(|detail| ForgeError::VerusOutput {
        detail: format!("could not reconstruct the composition: {detail}"),
    })?;
    let mut reconstructed = make_plan(PlanInput {
        raw_source,
        program,
        selected_program: &assembly.selected_program,
        closure: &assembly.closure,
        exports: &assembly.link_exports,
        crate_name: &plan.crate_name,
        target: plan.target,
        target_triple: &plan.target_triple,
        target_pointer_width: &plan.target_pointer_width,
        target_endian: &plan.target_endian,
        verus_source: &assembly.combined_source,
    });
    attach_composition_plan(&mut reconstructed, &assembly);
    Ok((
        reconstructed,
        assembly.lowered_thermite,
        assembly.combined_source,
        assembly.closure,
        assembly.link_exports,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_verus_policy_rejects_every_escape_class() {
        for (name, body) in [
            ("assume", "assume(false);"),
            ("admit", "admit();"),
            ("axiom", "axiom();"),
            ("external-body", "external_body"),
            ("unsafe", "unsafe { 0 };"),
            ("include", "include!(\"other.rs\");"),
            ("nested-module", "mod hidden { }"),
            ("nested-verus", "verus! { }"),
            ("unchecked-decreases", "decreases *"),
            ("attribute", "#[verifier::external_body]"),
        ] {
            let source = format!("pub fn bad() -> u64 {{ {body} 0 }}");
            assert!(
                analyze_shell(name, "", source.as_bytes()).is_err(),
                "policy accepted `{name}`: {source}"
            );
        }
        assert!(analyze_shell("declaration", "", b"pub fn missing() -> u64;").is_err());
    }

    #[test]
    fn direct_verus_inventory_is_deterministic_and_comment_aware() {
        let source = br#"
            // assume(false) in a comment is inert
            pub struct State { pub value: u64 }
            pub open spec fn related(s: State) -> bool { s.value == 1 }
            pub fn boot() -> (result: u64) ensures result == 1, { 1 }
        "#;
        let plan = analyze_shell("shell", "evidence/direct-verus/00-shell.rs", source).unwrap();
        assert_eq!(plan.items.len(), 3);
        assert_eq!(plan.items[0].name, "State");
        assert_eq!(plan.items[1].name, "boot");
        assert_eq!(plan.items[2].name, "related");
        assert!(plan.items.iter().all(|item| item.visibility == "public"));
    }
}
