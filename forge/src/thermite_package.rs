//! Canonical, receipt-bindable multi-file Thermite package loading.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cli::ForgeError;

pub const PACKAGE_SUFFIX: &str = ".thpkg.json";
pub const PACKAGE_SCHEMA: &str = "ThermitePackageV1";
pub const PACKAGE_EVIDENCE_DIR: &str = "evidence/thermite-package";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifestV1 {
    pub schema: String,
    pub name: String,
    pub modules: Vec<PackageModuleV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageModuleV1 {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedPackageModule {
    pub declaration: PackageModuleV1,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedPackage {
    pub manifest: PackageManifestV1,
    pub manifest_bytes: Vec<u8>,
    pub modules: Vec<LoadedPackageModule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedThermiteInput {
    pub bytes: Vec<u8>,
    pub package: Option<LoadedPackage>,
}

pub fn load(path: &Path) -> Result<LoadedThermiteInput, ForgeError> {
    if !is_package_path(path) {
        return Ok(LoadedThermiteInput {
            bytes: read_regular_file(path)?,
            package: None,
        });
    }
    let manifest_bytes = read_regular_file(path)?;
    let manifest = parse_canonical_manifest(&manifest_bytes)?;
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let canonical_root = fs::canonicalize(root).map_err(|source| ForgeError::Io {
        path: root.display().to_string(),
        source,
    })?;
    let mut modules = Vec::with_capacity(manifest.modules.len());
    for declaration in &manifest.modules {
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
                "module `{}` is not UTF-8: {error}",
                declaration.name
            ))
        })?;
        modules.push(LoadedPackageModule {
            declaration: declaration.clone(),
            bytes,
        });
    }
    let package = LoadedPackage {
        manifest,
        manifest_bytes,
        modules,
    };
    Ok(LoadedThermiteInput {
        bytes: combine(&package),
        package: Some(package),
    })
}

pub fn write_evidence(bundle_root: &Path, package: &LoadedPackage) -> Result<(), ForgeError> {
    let root = bundle_root.join(PACKAGE_EVIDENCE_DIR);
    fs::create_dir_all(root.join("source")).map_err(|source| ForgeError::Io {
        path: root.display().to_string(),
        source,
    })?;
    write_file(&root.join("manifest.json"), &package.manifest_bytes)?;
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

pub fn load_evidence(
    bundle_root: &Path,
    combined: &[u8],
) -> Result<Option<LoadedPackage>, ForgeError> {
    let root = bundle_root.join(PACKAGE_EVIDENCE_DIR);
    if !root.exists() {
        return Ok(None);
    }
    let manifest_bytes = read_regular_file(&root.join("manifest.json"))?;
    let manifest = parse_canonical_manifest(&manifest_bytes)?;
    let mut modules = Vec::with_capacity(manifest.modules.len());
    for declaration in &manifest.modules {
        let relative = validate_module_path(&declaration.path)?;
        let bytes = read_regular_file(&root.join("source").join(relative))?;
        std::str::from_utf8(&bytes).map_err(|error| {
            package_error(format!(
                "bound module `{}` is not UTF-8: {error}",
                declaration.name
            ))
        })?;
        modules.push(LoadedPackageModule {
            declaration: declaration.clone(),
            bytes,
        });
    }
    let package = LoadedPackage {
        manifest,
        manifest_bytes,
        modules,
    };
    if combine(&package) != combined {
        return Err(package_error(
            "bound package modules do not reconstruct evidence/input.th",
        ));
    }
    Ok(Some(package))
}

fn combine(package: &LoadedPackage) -> Vec<u8> {
    let mut combined = Vec::new();
    combined.extend_from_slice(
        format!(
            "// thermite-package schema={} name={}\n",
            package.manifest.schema, package.manifest.name
        )
        .as_bytes(),
    );
    for module in &package.modules {
        combined.extend_from_slice(
            format!(
                "// thermite-module name={} path={} sha256={}\n",
                module.declaration.name,
                module.declaration.path,
                sha256(&module.bytes)
            )
            .as_bytes(),
        );
        combined.extend_from_slice(&module.bytes);
        if !module.bytes.ends_with(b"\n") {
            combined.push(b'\n');
        }
    }
    combined
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
            "invalid package name `{}`",
            manifest.name
        )));
    }
    if manifest.modules.is_empty() {
        return Err(package_error(
            "a Thermite package must contain at least one module",
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
    }
    let mut canonical = serde_json::to_string_pretty(&manifest).map_err(|error| {
        package_error(format!("could not canonicalize package manifest: {error}"))
    })?;
    canonical.push('\n');
    if canonical.as_bytes() != bytes {
        return Err(package_error(
            "package manifest is not canonical pretty JSON with one trailing newline",
        ));
    }
    Ok(manifest)
}

fn is_package_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(PACKAGE_SUFFIX))
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
}

fn validate_module_path(path: &str) -> Result<PathBuf, ForgeError> {
    if path.is_empty() || path.contains('\\') || !path.ends_with(".th") {
        return Err(package_error(format!(
            "module path `{path}` must be a non-empty slash-separated .th path"
        )));
    }
    let relative = PathBuf::from(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(package_error(format!(
            "module path `{path}` is not a safe relative path"
        )));
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

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn package_error(detail: impl Into<String>) -> ForgeError {
    ForgeError::RustcOutput {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_NONCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn package_names_and_paths_are_closed() {
        assert!(valid_name("kernel_memory"));
        assert!(!valid_name("Kernel"));
        assert!(validate_module_path("memory/allocator.th").is_ok());
        assert!(validate_module_path("../allocator.th").is_err());
        assert!(validate_module_path("target/allocator.rs").is_err());
    }

    #[test]
    fn canonical_manifest_rejects_order_and_presentation_drift() {
        let reversed = br#"{
  "schema": "ThermitePackageV1",
  "name": "kernel",
  "modules": [
    {
      "name": "scheduler",
      "path": "scheduler.th"
    },
    {
      "name": "capability",
      "path": "capability.th"
    }
  ]
}
"#;
        assert!(parse_canonical_manifest(reversed).is_err());
        let compact = br#"{"schema":"ThermitePackageV1","name":"kernel","modules":[{"name":"capability","path":"capability.th"}]}"#;
        assert!(parse_canonical_manifest(compact).is_err());
    }

    #[test]
    fn modules_reconstruct_the_exact_bound_program() {
        let root = std::env::temp_dir().join(format!(
            "thermite-package-test-{}-{}",
            std::process::id(),
            TEST_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create package test root");
        fs::write(
            root.join("a.th"),
            b"fn a(x: u64) -> u64 req true ens result == x fx pure { x }\n",
        )
        .expect("write first module");
        fs::write(
            root.join("b.th"),
            b"fn b(x: u64) -> u64 req true ens result == x fx pure { a(x) }\n",
        )
        .expect("write second module");
        let manifest = PackageManifestV1 {
            schema: PACKAGE_SCHEMA.to_string(),
            name: "kernel".to_string(),
            modules: vec![
                PackageModuleV1 {
                    name: "a".to_string(),
                    path: "a.th".to_string(),
                },
                PackageModuleV1 {
                    name: "b".to_string(),
                    path: "b.th".to_string(),
                },
            ],
        };
        let mut manifest_bytes = serde_json::to_string_pretty(&manifest).unwrap();
        manifest_bytes.push('\n');
        let manifest_path = root.join("kernel.thpkg.json");
        fs::write(&manifest_path, manifest_bytes).expect("write manifest");

        let loaded = load(&manifest_path).expect("load package");
        let parsed = thermite_syntax::parse(std::str::from_utf8(&loaded.bytes).unwrap());
        assert!(parsed.is_clean(), "{:?}", parsed.errors);
        assert_eq!(parsed.program.items.len(), 2);

        let bundle = root.join("bundle");
        fs::create_dir(&bundle).expect("create bundle");
        write_evidence(&bundle, loaded.package.as_ref().unwrap()).expect("write evidence");
        let reconstructed = load_evidence(&bundle, &loaded.bytes)
            .expect("validate evidence")
            .expect("package evidence");
        assert_eq!(reconstructed, *loaded.package.as_ref().unwrap());
        fs::write(
            bundle.join(PACKAGE_EVIDENCE_DIR).join("source/a.th"),
            b"changed\n",
        )
        .expect("tamper module");
        assert!(load_evidence(&bundle, &loaded.bytes).is_err());

        fs::remove_dir_all(root).expect("remove package test root");
    }
}
