//! Canonical, source-identity-preserving Thermite package loading.
//!
//! [`load`] is the receipt-bound entry the L3 library and composition builds
//! consume: it returns the canonical backend projection plus the independently
//! parsed module closure. [`load_source`] is the front door every
//! source-oriented Forge command shares, so `check`, `audit`, the translation
//! validators, and the goal REPL resolve a `.thpkg.json` manifest through one
//! implementation. A single `.th` file keeps its read-and-parse behavior; a
//! manifest keeps each module's own path and span in every diagnostic, and
//! [`ResolvedSource::declaring_module`] maps an item back to the file that
//! declares it so a source rewrite lands there
//! (`.design/build/kernel-primitives.md`, "Modules, packages, and receipts").

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thermite_syntax::{parse_package, PackageModuleSource, PackageParseResult, Program};

use crate::cli::ForgeError;

pub const PACKAGE_SUFFIX: &str = ".thpkg.json";
pub const PACKAGE_SCHEMA: &str = "thermite.package.v1";
pub const PACKAGE_SOURCE_MAP_SCHEMA: &str = "thermite.package-source-map.v1";
pub const PACKAGE_EVIDENCE_DIR: &str = "evidence/thermite-package";

const MAX_MODULES: usize = 4096;
const MAX_NAME_BYTES: usize = 64;
const MAX_PATH_BYTES: usize = 4096;
const REJECTED_PATH_COMPONENTS: &[&str] = &["target", "dist", "__pycache__", ".git", ".hg", ".svn"];

/// The canonical package manifest. Module declarations and every nested import
/// list are strictly name-sorted; roots are a sorted, non-empty set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifestV1 {
    pub schema: String,
    pub name: String,
    pub roots: Vec<String>,
    pub modules: Vec<PackageModuleV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageModuleV1 {
    pub name: String,
    pub path: String,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedPackageModule {
    pub declaration: PackageModuleV1,
    pub bytes: Vec<u8>,
}

/// A map from canonical projection offsets back to exact module-local bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSourceMapV1 {
    pub schema: String,
    pub package: String,
    pub modules: Vec<PackageSourceMapModuleV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSourceMapModuleV1 {
    pub name: String,
    pub path: String,
    pub source_sha256: String,
    /// Byte offsets are fixed-width so canonical evidence is host-independent.
    pub projection_source_start: u64,
    pub source_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedPackage {
    pub manifest: PackageManifestV1,
    pub manifest_bytes: Vec<u8>,
    /// Dependency-first canonical module order.
    pub modules: Vec<LoadedPackageModule>,
    pub source_map: PackageSourceMapV1,
    pub source_map_bytes: Vec<u8>,
    /// Independently parsed package AST and module-local item origins.
    pub parsed: PackageParseResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedThermiteInput {
    /// Canonical backend projection. Original modules and its source map remain
    /// independently bound; this byte stream is not the source identity.
    pub bytes: Vec<u8>,
    pub package: Option<LoadedPackage>,
}

/// The declaring on-disk module of one top-level item, with its exact source.
///
/// A source-oriented command that rewrites source text splices into `source` at
/// the item's module-local span and writes the result back to `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaringModule {
    /// The file that declares the item. For a package this is the manifest's
    /// directory joined with the module's declared relative path.
    pub path: PathBuf,
    /// The exact module source the item's span indexes into.
    pub source: String,
}

/// One source-oriented Forge command's resolved input.
///
/// A single `.th` file resolves to its own text and AST. A canonical
/// `.thpkg.json` manifest resolves to the whole package closure: [`Self::program`]
/// carries module-local spans plus per-item module identity, and
/// [`Self::declaring_module`] maps an item back to the file that declares it.
/// [`Self::text`] is the canonical backend projection; [`Self::text_program`] is
/// its own parse, so those two are the pair to use when a consumer slices source
/// text by an item span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSource {
    path: PathBuf,
    program: Program,
    text: String,
    /// `None` for a single file, where `program` already indexes into `text`.
    text_program: Option<Program>,
    package: Option<LoadedPackage>,
}

impl ResolvedSource {
    /// The whole-program AST. Package spans are module-local and pair with
    /// [`Self::declaring_module`].
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The canonical source text [`Self::text_program`] spans index into.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The AST parsed from [`Self::text`]. For a single file this is
    /// [`Self::program`]; for a package it is the backend projection's parse,
    /// whose spans are offsets into the projection.
    #[must_use]
    pub fn text_program(&self) -> &Program {
        self.text_program.as_ref().unwrap_or(&self.program)
    }

    /// The file and exact source declaring `program().items[item_index]`.
    pub fn declaring_module(&self, item_index: usize) -> Result<DeclaringModule, ForgeError> {
        let Some(package) = &self.package else {
            return Ok(DeclaringModule {
                path: self.path.clone(),
                source: self.text.clone(),
            });
        };
        let origin = package.parsed.origin(item_index).ok_or_else(|| {
            package_error(format!(
                "package item index {item_index} has no recorded module origin"
            ))
        })?;
        let module = package
            .modules
            .iter()
            .find(|module| module.declaration.path == origin.path)
            .ok_or_else(|| {
                package_error(format!(
                    "module `{}` ({}) is not bound in the loaded package",
                    origin.module, origin.path
                ))
            })?;
        let source = String::from_utf8(module.bytes.clone()).map_err(|error| {
            package_error(format!(
                "module `{}` ({}) is not UTF-8: {error}",
                origin.module, origin.path
            ))
        })?;
        let relative = validate_module_path(&origin.path)?;
        let root = self.path.parent().unwrap_or_else(|| Path::new("."));
        Ok(DeclaringModule {
            path: root.join(relative),
            source,
        })
    }
}

/// Resolve one source-oriented command's path argument.
///
/// A path that is not a canonical `.thpkg.json` manifest keeps the single-file
/// read-and-parse behavior verbatim: the file bytes, `thermite_syntax::parse`,
/// and a `ForgeError::Parse` carrying the recovered syntax errors. A manifest
/// routes through [`load`], so each declared module is parsed on its own and a
/// syntax error names that module's path and a span inside it.
pub fn load_source(path: &Path) -> Result<ResolvedSource, ForgeError> {
    if !is_package_path(path) {
        let text = fs::read_to_string(path).map_err(|source| ForgeError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let parsed = thermite_syntax::parse(&text);
        if !parsed.is_clean() {
            return Err(ForgeError::Parse(parsed.errors));
        }
        return Ok(ResolvedSource {
            path: path.to_path_buf(),
            program: parsed.program,
            text,
            text_program: None,
            package: None,
        });
    }

    let loaded = load(path)?;
    let text = String::from_utf8(loaded.bytes).map_err(|error| {
        package_error(format!(
            "canonical package projection is not UTF-8: {error}"
        ))
    })?;
    let Some(package) = loaded.package else {
        return Err(package_error(
            "package loading produced no module closure for a manifest path",
        ));
    };
    let projected = thermite_syntax::parse(&text);
    if !projected.is_clean() {
        return Err(ForgeError::Parse(projected.errors));
    }
    let program = package.parsed.program.clone();
    if !declares_same_items(&program, &projected.program) {
        return Err(package_error(
            "independent module parsing disagrees with the canonical backend projection",
        ));
    }
    Ok(ResolvedSource {
        path: path.to_path_buf(),
        program,
        text,
        text_program: Some(projected.program),
        package: Some(package),
    })
}

/// Whether two parses declare the same items in the same order. The projection
/// parse is used only to pair spans with projection text, so it is bound to the
/// module-local parse by this agreement check.
fn declares_same_items(left: &Program, right: &Program) -> bool {
    left.items.len() == right.items.len()
        && left
            .items
            .iter()
            .zip(right.items.iter())
            .all(|(left, right)| left.name() == right.name())
}

/// Load either a legacy single `.th` file or a canonical `.thpkg.json` package.
pub fn load(path: &Path) -> Result<LoadedThermiteInput, ForgeError> {
    if !is_package_path(path) {
        return Ok(LoadedThermiteInput {
            bytes: read_regular_file(path)?,
            package: None,
        });
    }

    let manifest_bytes = read_regular_file(path)?;
    let manifest = parse_canonical_manifest(&manifest_bytes)?;
    let module_order = validate_graph(&manifest)?;
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let canonical_root = fs::canonicalize(root).map_err(|source| ForgeError::Io {
        path: root.display().to_string(),
        source,
    })?;
    let declarations: BTreeMap<&str, &PackageModuleV1> = manifest
        .modules
        .iter()
        .map(|module| (module.name.as_str(), module))
        .collect();
    let mut modules = Vec::with_capacity(module_order.len());
    for name in module_order {
        let declaration = declarations[name.as_str()];
        let relative = validate_module_path(&declaration.path)?;
        reject_symlink_components(root, &relative)?;
        let source_path = root.join(&relative);
        let canonical_source = fs::canonicalize(&source_path).map_err(|source| ForgeError::Io {
            path: source_path.display().to_string(),
            source,
        })?;
        if !canonical_source.starts_with(&canonical_root) {
            return Err(package_error(format!(
                "module `{}` escapes the package directory",
                declaration.name
            )));
        }
        let bytes = read_regular_file(&source_path)?;
        std::str::from_utf8(&bytes).map_err(|error| {
            package_error(format!(
                "module `{}` ({}) is not UTF-8: {error}",
                declaration.name, declaration.path
            ))
        })?;
        modules.push(LoadedPackageModule {
            declaration: declaration.clone(),
            bytes,
        });
    }

    finish_loaded_package(manifest, manifest_bytes, modules)
}

fn finish_loaded_package(
    manifest: PackageManifestV1,
    manifest_bytes: Vec<u8>,
    modules: Vec<LoadedPackageModule>,
) -> Result<LoadedThermiteInput, ForgeError> {
    let sources: Vec<PackageModuleSource> = modules
        .iter()
        .map(|module| PackageModuleSource {
            name: module.declaration.name.clone(),
            path: module.declaration.path.clone(),
            source: String::from_utf8(module.bytes.clone())
                .expect("module UTF-8 was checked before package assembly"),
        })
        .collect();
    let parsed = parse_package(&sources);
    if !parsed.is_clean() {
        let detail = parsed
            .errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(package_error(detail));
    }
    let (bytes, source_map) = project(&manifest, &modules);
    let source_map_bytes = canonical_json(&source_map, "package source map")?;
    let package = LoadedPackage {
        manifest,
        manifest_bytes,
        modules,
        source_map,
        source_map_bytes,
        parsed,
    };
    Ok(LoadedThermiteInput {
        bytes,
        package: Some(package),
    })
}

/// Write every original module, the canonical manifest, and the projection map
/// into a verified bundle. The bundle's ordinary file inventory binds these
/// bytes; no directory walk is used to discover source inputs.
pub fn write_evidence(bundle_root: &Path, package: &LoadedPackage) -> Result<(), ForgeError> {
    let root = bundle_root.join(PACKAGE_EVIDENCE_DIR);
    fs::create_dir_all(root.join("source")).map_err(|source| ForgeError::Io {
        path: root.display().to_string(),
        source,
    })?;
    write_file(&root.join("manifest.json"), &package.manifest_bytes)?;
    write_file(&root.join("source-map.json"), &package.source_map_bytes)?;
    for module in &package.modules {
        let relative = validate_module_path(&module.declaration.path)?;
        let destination = root.join("source").join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| ForgeError::Io {
                path: parent.display().to_string(),
                source,
            })?;
        }
        write_file(&destination, &module.bytes)?;
    }
    Ok(())
}

/// Reconstruct and independently reparse package evidence during validation.
/// Returns `None` for a legacy single-file bundle.
pub fn load_evidence(
    bundle_root: &Path,
    projection: &[u8],
) -> Result<Option<LoadedPackage>, ForgeError> {
    let root = bundle_root.join(PACKAGE_EVIDENCE_DIR);
    if !root.exists() {
        return Ok(None);
    }
    let manifest_bytes = read_regular_file(&root.join("manifest.json"))?;
    let manifest = parse_canonical_manifest(&manifest_bytes)?;
    let module_order = validate_graph(&manifest)?;
    let declarations: BTreeMap<&str, &PackageModuleV1> = manifest
        .modules
        .iter()
        .map(|module| (module.name.as_str(), module))
        .collect();
    let mut modules = Vec::with_capacity(module_order.len());
    for name in module_order {
        let declaration = declarations[name.as_str()];
        let relative = validate_module_path(&declaration.path)?;
        let bytes = read_regular_file(&root.join("source").join(relative))?;
        std::str::from_utf8(&bytes).map_err(|error| {
            package_error(format!(
                "bound module `{}` ({}) is not UTF-8: {error}",
                declaration.name, declaration.path
            ))
        })?;
        modules.push(LoadedPackageModule {
            declaration: declaration.clone(),
            bytes,
        });
    }
    let loaded = finish_loaded_package(manifest, manifest_bytes, modules)?;
    if loaded.bytes != projection {
        return Err(package_error(
            "bound package modules do not reconstruct evidence/input.th",
        ));
    }
    let package = loaded
        .package
        .expect("package assembly always returns a package");
    let bound_source_map = read_regular_file(&root.join("source-map.json"))?;
    if bound_source_map != package.source_map_bytes {
        return Err(package_error(
            "bound package source map is not the canonical map for its modules",
        ));
    }
    Ok(Some(package))
}

fn project(
    manifest: &PackageManifestV1,
    modules: &[LoadedPackageModule],
) -> (Vec<u8>, PackageSourceMapV1) {
    let mut projection = format!(
        "// thermite-package schema={} name={}\n",
        manifest.schema, manifest.name
    )
    .into_bytes();
    let mut mapped = Vec::with_capacity(modules.len());
    for module in modules {
        let source_sha256 = sha256(&module.bytes);
        projection.extend_from_slice(
            format!(
                "// thermite-module name={} path={} sha256={}\n",
                module.declaration.name, module.declaration.path, source_sha256
            )
            .as_bytes(),
        );
        let projection_source_start = projection.len() as u64;
        projection.extend_from_slice(&module.bytes);
        mapped.push(PackageSourceMapModuleV1 {
            name: module.declaration.name.clone(),
            path: module.declaration.path.clone(),
            source_sha256,
            projection_source_start,
            source_len: module.bytes.len() as u64,
        });
        if !module.bytes.ends_with(b"\n") {
            projection.push(b'\n');
        }
    }
    (
        projection,
        PackageSourceMapV1 {
            schema: PACKAGE_SOURCE_MAP_SCHEMA.to_string(),
            package: manifest.name.clone(),
            modules: mapped,
        },
    )
}

fn parse_canonical_manifest(bytes: &[u8]) -> Result<PackageManifestV1, ForgeError> {
    let manifest: PackageManifestV1 = serde_json::from_slice(bytes)
        .map_err(|error| package_error(format!("invalid package manifest: {error}")))?;
    if manifest.schema != PACKAGE_SCHEMA {
        return Err(package_error(format!(
            "unsupported package schema `{}`",
            manifest.schema
        )));
    }
    if !valid_name(&manifest.name) {
        return Err(package_error(format!(
            "invalid package name `{}`; expected [a-z][a-z0-9_]* up to {MAX_NAME_BYTES} bytes",
            manifest.name
        )));
    }
    if manifest.modules.is_empty() || manifest.modules.len() > MAX_MODULES {
        return Err(package_error(format!(
            "a package must declare between 1 and {MAX_MODULES} modules"
        )));
    }
    ensure_sorted_unique(&manifest.roots, "package roots")?;
    if manifest.roots.is_empty() {
        return Err(package_error(
            "a package must declare at least one root module",
        ));
    }
    let mut previous = None;
    let mut paths = BTreeSet::new();
    for module in &manifest.modules {
        if !valid_name(&module.name) {
            return Err(package_error(format!(
                "invalid module name `{}`",
                module.name
            )));
        }
        if previous.is_some_and(|name: &str| name >= module.name.as_str()) {
            return Err(package_error(
                "package modules must be strictly sorted by unique name",
            ));
        }
        previous = Some(module.name.as_str());
        validate_module_path(&module.path)?;
        if !paths.insert(module.path.as_str()) {
            return Err(package_error(format!(
                "duplicate package module path `{}`",
                module.path
            )));
        }
        ensure_sorted_unique(
            &module.imports,
            &format!("imports of module `{}`", module.name),
        )?;
        for import in &module.imports {
            if !valid_name(import) {
                return Err(package_error(format!(
                    "module `{}` has invalid import name `{import}`",
                    module.name
                )));
            }
        }
    }
    let canonical = canonical_json(&manifest, "package manifest")?;
    if canonical != bytes {
        return Err(package_error(
            "package manifest is not canonical pretty JSON with one trailing newline",
        ));
    }
    Ok(manifest)
}

/// Validate imports, reject cycles/unreachable declarations, and return a
/// dependency-first deterministic module order.
fn validate_graph(manifest: &PackageManifestV1) -> Result<Vec<String>, ForgeError> {
    let modules: BTreeMap<&str, &PackageModuleV1> = manifest
        .modules
        .iter()
        .map(|module| (module.name.as_str(), module))
        .collect();
    for root in &manifest.roots {
        if !modules.contains_key(root.as_str()) {
            return Err(package_error(format!("unknown root module `{root}`")));
        }
    }
    for module in &manifest.modules {
        for import in &module.imports {
            if !modules.contains_key(import.as_str()) {
                return Err(package_error(format!(
                    "module `{}` imports unknown module `{import}`",
                    module.name
                )));
            }
        }
    }

    let mut state: BTreeMap<&str, u8> = modules.keys().map(|name| (*name, 0)).collect();
    let mut stack = Vec::new();
    let mut order = Vec::with_capacity(modules.len());
    for root in &manifest.roots {
        visit_module(root, &modules, &mut state, &mut stack, &mut order)?;
    }
    let unreachable: Vec<&str> = state
        .iter()
        .filter_map(|(name, state)| (*state == 0).then_some(*name))
        .collect();
    if !unreachable.is_empty() {
        return Err(package_error(format!(
            "declared module(s) are unreachable from package roots: {}",
            unreachable.join(", ")
        )));
    }
    Ok(order)
}

fn visit_module<'a>(
    name: &'a str,
    modules: &BTreeMap<&'a str, &'a PackageModuleV1>,
    state: &mut BTreeMap<&'a str, u8>,
    stack: &mut Vec<&'a str>,
    order: &mut Vec<String>,
) -> Result<(), ForgeError> {
    match state[name] {
        2 => return Ok(()),
        1 => {
            let start = stack.iter().position(|entry| *entry == name).unwrap_or(0);
            let mut cycle = stack[start..].to_vec();
            cycle.push(name);
            return Err(package_error(format!(
                "module import cycle: {}",
                cycle.join(" -> ")
            )));
        }
        _ => {}
    }
    state.insert(name, 1);
    stack.push(name);
    for import in &modules[name].imports {
        visit_module(import, modules, state, stack, order)?;
    }
    let popped = stack.pop();
    debug_assert_eq!(popped, Some(name));
    state.insert(name, 2);
    order.push(name.to_string());
    Ok(())
}

fn ensure_sorted_unique(values: &[String], label: &str) -> Result<(), ForgeError> {
    if values
        .windows(2)
        .any(|window| window[0].as_str() >= window[1].as_str())
    {
        return Err(package_error(format!(
            "{label} must be strictly sorted and unique"
        )));
    }
    Ok(())
}

pub fn is_package_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(PACKAGE_SUFFIX))
}

fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return false;
    }
    let mut chars = name.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
}

fn validate_module_path(path: &str) -> Result<PathBuf, ForgeError> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.contains('\\')
        || !path.ends_with(".th")
    {
        return Err(package_error(format!(
            "module path `{path}` must be a non-empty slash-separated .th path up to {MAX_PATH_BYTES} bytes"
        )));
    }
    let relative = PathBuf::from(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(package_error(format!(
            "module path `{path}` is not a safe normalized relative path"
        )));
    }
    for component in relative.components() {
        let Component::Normal(component) = component else {
            unreachable!("component shape was checked above")
        };
        let component = component.to_str().ok_or_else(|| {
            package_error(format!(
                "module path `{path}` contains a non-UTF-8 component"
            ))
        })?;
        if REJECTED_PATH_COMPONENTS.contains(&component) {
            return Err(package_error(format!(
                "module path `{path}` contains rejected generated/incidental component `{component}`"
            )));
        }
    }
    Ok(relative)
}

fn reject_symlink_components(root: &Path, relative: &Path) -> Result<(), ForgeError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(package_error("package module path is not normalized"));
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|source| ForgeError::Io {
            path: current.display().to_string(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(package_error(format!(
                "package path component `{}` is a symlink",
                current.display()
            )));
        }
    }
    Ok(())
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, ForgeError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    if !metadata.file_type().is_file() {
        return Err(package_error(format!(
            "package input `{}` is not a regular file",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ForgeError> {
    fs::write(path, bytes).map_err(|source| ForgeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn canonical_json<T: Serialize>(value: &T, label: &str) -> Result<Vec<u8>, ForgeError> {
    let mut rendered = serde_json::to_string_pretty(value)
        .map_err(|error| package_error(format!("could not serialize {label}: {error}")))?;
    rendered.push('\n');
    Ok(rendered.into_bytes())
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn package_error(detail: impl Into<String>) -> ForgeError {
    ForgeError::Package {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_NONCE: AtomicU64 = AtomicU64::new(0);

    fn canonical_manifest(manifest: &PackageManifestV1) -> Vec<u8> {
        canonical_json(manifest, "test manifest").unwrap()
    }

    fn fixture_manifest() -> PackageManifestV1 {
        PackageManifestV1 {
            schema: PACKAGE_SCHEMA.to_string(),
            name: "primitives".to_string(),
            roots: vec!["api".to_string()],
            modules: vec![
                PackageModuleV1 {
                    name: "api".to_string(),
                    path: "src/api.th".to_string(),
                    imports: vec!["base".to_string()],
                },
                PackageModuleV1 {
                    name: "base".to_string(),
                    path: "src/base.th".to_string(),
                    imports: vec![],
                },
            ],
        }
    }

    #[test]
    fn names_paths_and_generated_directories_are_closed() {
        assert!(valid_name("memory_primitives"));
        assert!(!valid_name("Kernel"));
        assert!(validate_module_path("memory/atomic.th").is_ok());
        assert!(validate_module_path("../atomic.th").is_err());
        assert!(validate_module_path("target/atomic.th").is_err());
        assert!(validate_module_path("src/__pycache__/atomic.th").is_err());
        assert!(validate_module_path("dist/atomic.th").is_err());
    }

    #[test]
    fn canonical_manifest_rejects_cycles_unknowns_and_unreachable_modules() {
        let mut manifest = fixture_manifest();
        manifest.modules[1].imports = vec!["api".to_string()];
        assert!(validate_graph(&manifest)
            .unwrap_err()
            .to_string()
            .contains("api -> base -> api"));

        let mut manifest = fixture_manifest();
        manifest.modules[0].imports = vec!["missing".to_string()];
        assert!(validate_graph(&manifest)
            .unwrap_err()
            .to_string()
            .contains("unknown"));

        let mut manifest = fixture_manifest();
        manifest.modules.push(PackageModuleV1 {
            name: "unused".to_string(),
            path: "src/unused.th".to_string(),
            imports: vec![],
        });
        assert!(validate_graph(&manifest)
            .unwrap_err()
            .to_string()
            .contains("unreachable"));
    }

    #[test]
    fn manifest_presentation_and_order_are_canonical() {
        let manifest = fixture_manifest();
        let bytes = canonical_manifest(&manifest);
        assert_eq!(parse_canonical_manifest(&bytes).unwrap(), manifest);
        let compact = serde_json::to_vec(&manifest).unwrap();
        assert!(parse_canonical_manifest(&compact).is_err());
        let mut reversed = manifest;
        reversed.modules.reverse();
        assert!(parse_canonical_manifest(&canonical_manifest(&reversed)).is_err());
    }

    #[test]
    fn evidence_reconstructs_original_modules_projection_and_source_map() {
        let root = std::env::temp_dir().join(format!(
            "thermite-package-v1-{}-{}",
            std::process::id(),
            TEST_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(
            root.join("src/base.th"),
            b"fn base(x: u64) -> u64 req true ens result == x fx pure { x }\n",
        )
        .unwrap();
        fs::write(
            root.join("src/api.th"),
            b"fn api(x: u64) -> u64 req true ens result == x fx pure { base(x) }\n",
        )
        .unwrap();
        let manifest = fixture_manifest();
        let manifest_path = root.join("primitives.thpkg.json");
        fs::write(&manifest_path, canonical_manifest(&manifest)).unwrap();

        let loaded = load(&manifest_path).unwrap();
        let package = loaded.package.as_ref().unwrap();
        assert_eq!(package.modules[0].declaration.name, "base");
        assert_eq!(package.modules[1].declaration.name, "api");
        assert_eq!(package.parsed.origin(0).unwrap().path, "src/base.th");
        assert_eq!(package.parsed.origin(1).unwrap().path, "src/api.th");

        let bundle = root.join("bundle");
        fs::create_dir(&bundle).unwrap();
        write_evidence(&bundle, package).unwrap();
        let reconstructed = load_evidence(&bundle, &loaded.bytes).unwrap().unwrap();
        assert_eq!(reconstructed, *package);

        fs::write(
            bundle.join(PACKAGE_EVIDENCE_DIR).join("source/src/base.th"),
            b"changed\n",
        )
        .unwrap();
        assert!(load_evidence(&bundle, &loaded.bytes).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
