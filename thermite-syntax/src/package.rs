//! Source-identity-preserving parsing for receipt-bound Thermite packages.
//!
//! A package manifest and its dependency graph are owned by Forge. This module
//! owns the syntax boundary: each declared module is parsed independently, its
//! spans remain relative to that module, and only then are the recovered items
//! assembled into the whole-package [`Program`] consumed by validation and
//! lowering. No anonymous text concatenation is involved.

use std::collections::BTreeMap;
use std::fmt;

use crate::{parse, ForgeItem, Item, Program, Span, SyntaxError};

/// One manifest-declared module presented to the package parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageModuleSource {
    /// Canonical semantic module name.
    pub name: String,
    /// Canonical package-relative source path.
    pub path: String,
    /// Exact UTF-8 source text.
    pub source: String,
}

/// The source identity of one parsed top-level item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemOrigin {
    pub module: String,
    pub path: String,
    /// A span into this origin's module source, never a package-wide offset.
    pub span: Span,
}

/// Per-module range into [`PackageParseResult::program`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedModule {
    pub name: String,
    pub path: String,
    pub first_item: usize,
    pub item_count: usize,
}

/// A package-local syntax or deterministic name-resolution diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageParseError {
    Syntax {
        module: String,
        path: String,
        error: SyntaxError,
    },
    DuplicateItem {
        name: String,
        first: ItemOrigin,
        duplicate: ItemOrigin,
    },
}

impl fmt::Display for PackageParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageParseError::Syntax {
                module,
                path,
                error,
            } => write!(f, "module `{module}` ({path}): {error}"),
            PackageParseError::DuplicateItem {
                name,
                first,
                duplicate,
            } => write!(
                f,
                "duplicate package item `{name}`: first declared in module `{}` ({}) at byte {}, repeated in module `{}` ({}) at byte {}",
                first.module,
                first.path,
                first.span.start,
                duplicate.module,
                duplicate.path,
                duplicate.span.start,
            ),
        }
    }
}

impl std::error::Error for PackageParseError {}

/// Whole-package parse result. `item_origins` is positionally aligned with
/// `program.items`, including the surviving items of modules with syntax errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageParseResult {
    pub program: Program,
    pub modules: Vec<ParsedModule>,
    pub item_origins: Vec<ItemOrigin>,
    pub errors: Vec<PackageParseError>,
}

impl PackageParseResult {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }

    /// Resolve a whole-package item index back to its exact source module/span.
    #[must_use]
    pub fn origin(&self, item_index: usize) -> Option<&ItemOrigin> {
        self.item_origins.get(item_index)
    }
}

/// Parse manifest-ordered modules independently and assemble their item ASTs.
///
/// Name resolution in the current surface is a single package namespace, so a
/// duplicate top-level name is rejected deterministically at the second
/// declaration. Dependency reachability and cycle checks happen before this
/// function in the manifest loader.
#[must_use]
pub fn parse_package(sources: &[PackageModuleSource]) -> PackageParseResult {
    let mut items = Vec::new();
    let mut modules = Vec::with_capacity(sources.len());
    let mut item_origins = Vec::new();
    let mut errors = Vec::new();
    let mut declarations: BTreeMap<String, ItemOrigin> = BTreeMap::new();

    for source in sources {
        let parsed = parse(&source.source);
        errors.extend(
            parsed
                .errors
                .into_iter()
                .map(|error| PackageParseError::Syntax {
                    module: source.name.clone(),
                    path: source.path.clone(),
                    error,
                }),
        );

        let first_item = items.len();
        for item in parsed.program.items {
            let origin = ItemOrigin {
                module: source.name.clone(),
                path: source.path.clone(),
                span: item.span(),
            };
            if let Some(name) = declaration_name(&item) {
                if let Some(first) = declarations.get(name) {
                    errors.push(PackageParseError::DuplicateItem {
                        name: name.to_string(),
                        first: first.clone(),
                        duplicate: origin.clone(),
                    });
                } else {
                    declarations.insert(name.to_string(), origin.clone());
                }
            }
            item_origins.push(origin);
            items.push(item);
        }
        modules.push(ParsedModule {
            name: source.name.clone(),
            path: source.path.clone(),
            first_item,
            item_count: items.len() - first_item,
        });
    }

    PackageParseResult {
        program: Program { items },
        modules,
        item_origins,
        errors,
    }
}

/// Return the name introduced into the package's declaration namespace.
/// `proof for` and `witness` items have semantic address roots, but do not
/// introduce symbols: a proof intentionally shares its target function's name,
/// and multiple witness blocks are numbered by the address layer.
fn declaration_name(item: &Item) -> Option<&str> {
    match item {
        Item::Fn(function) => Some(&function.name),
        Item::SpecFn(function) => Some(&function.name),
        Item::Struct(structure) => Some(&structure.name),
        Item::Enum(enumeration) => Some(&enumeration.name),
        Item::Forge(ForgeItem::PropFn(function)) => Some(&function.name),
        Item::Forge(ForgeItem::Lemma(lemma)) => Some(&lemma.name),
        Item::Forge(ForgeItem::Proof(_) | ForgeItem::Witness(_)) => None,
    }
}
