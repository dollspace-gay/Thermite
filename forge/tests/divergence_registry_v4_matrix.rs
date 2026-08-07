//! Divergence battery for the frozen-primitive-registry machine-class matrix
//! (REQ-KPRIM-5 / REQ-KPRIM-7 / REQ-KPRIM-9).
//!
//! Authority chain:
//!
//! * `.design/build/frozen-primitive-registry.md` §Scope — "It rejects
//!   otherwise well-formed `atomic`, `volatile`, and `privileged` entries
//!   because safe-Rust source/object closure does not model their machine
//!   semantics."
//! * `.design/build/frozen-primitive-registry.md` §"Schema v3: machine-aware
//!   atomic pilot" — "A v3 bundle is consequently reported as
//!   `L1/to_machine_boundary`; it is never laundered into an end-to-end L3
//!   artifact."
//! * `.design/build/platform-primitives.md` §"Consumer refinement rule" — "The
//!   safe sequential registry-v2 path must reject them rather than laundering
//!   their contracts through a safe Rust model."
//! * `.design/build/kernel-primitives.md` §"Atomics and memory ordering" —
//!   "Safe v1/v2 linkages refuse to overstate that missing assurance, while the
//!   v3 pilot remains explicitly residual."
//! * `.design/build/kernel-primitives.md` §"Acceptance matrix" — "Compose a
//!   synthetic test platform whose bodies are tiny direct-Verus adapters. This
//!   exercises the registry/refinement machinery without booting or
//!   implementing a kernel."
//! * `.design/build/frozen-primitive-registry.md` §"Schema v1" — each entry
//!   declares "parameter and result ownership (`by_value`, shared/exclusive
//!   borrow, sealed consume, or sealed mint) derived independently from the
//!   AST".
//!
//! R-CHAR-3: every expected value below is either quoted design text or read
//! out of a tracked conformance file (`machine_atomic_registry.json`,
//! `machine_atomic.th`, `platform/api.th`). No expectation is copied from
//! forge's own output.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

const REGISTRY_SCHEMA_PREFIX: &str = "thermite.frozen-primitive-registry.";

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

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_divergence_registry_v4_{name}_{}_{}",
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

/// The tracked v3 pilot registry is the authority for the `machine_atomic.th`
/// declaration's normalized signature, contract digest, effect digest, and
/// effect row. Reading them here keeps the laundering probe honest: the only
/// thing this test changes relative to the tracked pilot is the *linkage and
/// concurrency claim*, never the Thermite source facts.
fn pilot_entry() -> serde_json::Value {
    let bytes =
        fs::read(root().join("conformance/verified-composition/machine_atomic_registry.json"))
            .unwrap();
    let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        document["schema"], "thermite.frozen-primitive-registry.v3",
        "the tracked machine pilot is the v3 authority for this probe"
    );
    assert_eq!(
        document["entries"][0]["effects"],
        serde_json::json!(["platform(atomic)"]),
        "the tracked machine pilot door must carry the frozen platform(atomic) effect atom"
    );
    document["entries"][0].clone()
}

/// Build `conformance/verified-composition/machine_atomic.th` — the tracked
/// `fx platform(atomic)` machine door — against a *safe* registry linkage whose
/// implementation is an ordinary `{ value }` Rust body, and return the CLI
/// result plus the bundle path.
fn build_laundered_atomic(
    temp: &TempDir,
    schema: &str,
    linkage: Option<&str>,
    refinement: &str,
    obligations: &[&str],
    crate_name: &str,
) -> (Output, PathBuf) {
    // A tiny safe direct-Verus adapter with no atomic operation whatsoever.
    let safe_impl = b"pub fn atomic_roundtrip_impl(value: u64) -> (result: u64)\n    ensures result == value,\n{\n    value\n}\n";
    let safe_impl_path = temp.0.join("safe_atomic_impl.rs");
    fs::write(&safe_impl_path, safe_impl).unwrap();

    let pilot = pilot_entry();
    let mut implementation = serde_json::json!({
        "shell_module": "safe_atomic_impl",
        "item": "atomic_roundtrip_impl",
        "source_sha256": digest(safe_impl),
        "abi": "Rust",
        "symbol": "safe_atomic_impl::atomic_roundtrip_impl",
        "alignment": 8,
    });
    if let Some(linkage) = linkage {
        implementation["linkage"] = serde_json::json!(linkage);
    }
    let registry = serde_json::json!({
        "schema": schema,
        "target": {
            "target_triple": "x86_64-unknown-linux-gnu",
            "target_features": ["sse2"],
        },
        "entries": [{
            "semantic_name": "synthetic::atomic.u64.seq_cst_roundtrip@v1",
            "version": 1,
            "required": true,
            // Every field below is the tracked v3 pilot's own source-derived value.
            "thermite_name": pilot["thermite_name"],
            "boundary_target": pilot["boundary_target"],
            "signature": pilot["signature"],
            "contract_sha256": pilot["contract_sha256"],
            "effects_sha256": pilot["effects_sha256"],
            "effects": pilot["effects"],
            "ownership": pilot["ownership"],
            "implementation": implementation,
            "model": "thermite_contract",
            "refinement": refinement,
            "proof_obligations": obligations,
            // The laundering claim: a `platform(atomic)` door declared sequential.
            "concurrency": "sequential",
            "memory_orderings": [],
            "failure": "total",
        }],
    });
    let registry_path = temp.0.join("laundered.registry.json");
    fs::write(
        &registry_path,
        serde_json::to_vec_pretty(&registry).unwrap(),
    )
    .unwrap();

    let bundle = temp.0.join("laundered.verified");
    let output = forge(&[
        "build",
        "conformance/verified-composition/machine_atomic.th",
        "--level",
        "l3",
        "--compose-export",
        "machine_atomic_observation",
        "--compose-shell",
        safe_impl_path.to_string_lossy().as_ref(),
        "--compose-shell",
        "conformance/verified-composition/machine_atomic_shell.rs",
        "--primitive-registry",
        registry_path.to_string_lossy().as_ref(),
        "--crate-name",
        crate_name,
        "--target",
        "kernel",
        "--out",
        bundle.to_string_lossy().as_ref(),
        "--json",
    ]);
    (output, bundle)
}

/// `.design/build/frozen-primitive-registry.md` §"Schema v3": a machine-class
/// crossing "is never laundered into an end-to-end L3 artifact". The authority
/// therefore permits exactly two outcomes for a safe-linkage entry over a
/// `fx platform(atomic)` door: rejection (the §Scope / platform-primitives.md
/// "must reject" rule), or publication under the honest v3-style cap
/// (`L1` / `to_machine_boundary` with a visible residual assumption).
fn assert_machine_door_is_not_laundered(label: &str, output: &Output, bundle: &Path) {
    if !output.status.success() {
        assert!(
            !bundle.exists(),
            "{label}: a rejected machine-class registry still published `{}`",
            bundle.display()
        );
        return;
    }
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("receipt.json")).unwrap()).unwrap();
    let binding = &receipt["binding"];
    let members = binding["assurance_aggregate"]["members"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let boundary_member = members
        .iter()
        .find(|member| member["name"] == "machine_atomic_roundtrip")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    assert!(
        !(binding["assurance"] == "L3" && binding["scope"] == "end_to_end"),
        "{label}: the safe registry linkage published `fx platform(atomic)` boundary \
         `machine_atomic_roundtrip` as assurance={} scope={}. \
         `.design/build/frozen-primitive-registry.md` §\"Schema v3\": a machine crossing \
         \"is never laundered into an end-to-end L3 artifact\"; \
         `.design/build/platform-primitives.md` §\"Consumer refinement rule\": \
         \"The safe sequential registry-v2 path must reject them rather than laundering \
         their contracts through a safe Rust model.\" \
         boundary member = {boundary_member}, \
         residual_machine_assumptions = {}",
        binding["assurance"],
        binding["scope"],
        binding["composition"]["residual_machine_assumptions"],
    );
    assert_ne!(
        boundary_member["achieved"], "L3-direct-refinement",
        "{label}: `machine_atomic_roundtrip` carries `fx platform(atomic)` yet reached \
         `L3-direct-refinement` through a safe `{{ value }}` Rust body. \
         `.design/build/kernel-primitives.md` §\"Atomics and memory ordering\": \
         \"Safe v1/v2 linkages refuse to overstate that missing assurance.\""
    );
    assert!(
        binding["composition"]["residual_machine_assumptions"]
            .as_u64()
            .unwrap_or(0)
            >= 1,
        "{label}: a `fx platform(atomic)` crossing published with \
         residual_machine_assumptions = {}. \
         `.design/build/frozen-primitive-registry.md` §\"Exact checked-wrapper refinement\": \
         \"the bodyless Thermite boundary remains L1 and caps the complete artifact at \
         `L1/to_machine_boundary`.\"",
        binding["composition"]["residual_machine_assumptions"],
    );
}

/// Divergence: `forge/src/verified_build/primitive_registry.rs::plan_from_bytes`
/// decides the machine class from the registry's *self-declared*
/// `entry.concurrency` string alone. It never consults the source-derived
/// frozen platform-effect row (`.design/build/kernel-primitives.md` §"Sealed
/// authority and platform effects" freezes that family), so a consumer registry
/// that spells `"concurrency": "sequential"` over a `fx platform(atomic)` door
/// takes the safe same-crate v1 path and is published as an end-to-end L3
/// artifact.
///
/// Input: the tracked `conformance/verified-composition/machine_atomic.th`
/// declaration `machine_atomic_roundtrip` (`fx platform(atomic)`), bound to a
/// direct-Verus body containing no atomic operation at all.
#[test]
fn divergence_safe_v1_registry_launders_a_platform_atomic_machine_door() {
    let temp = TempDir::new("v1-launder");
    let (output, bundle) = build_laundered_atomic(
        &temp,
        "thermite.frozen-primitive-registry.v1",
        None,
        "same_crate_verus_checked_wrapper",
        &[
            "contract_refinement",
            "exact_implementation_call",
            "whole_crate_no_cheating",
        ],
        "thermite_divergence_laundered_atomic_v1",
    );
    assert_machine_door_is_not_laundered("registry v1 same_crate", &output, &bundle);
}

/// The same divergence through the v2 `separate_verus_crate` linkage, which is
/// the exact path `.design/build/platform-primitives.md` §"Consumer refinement
/// rule" names: "The safe sequential registry-v2 path must reject them rather
/// than laundering their contracts through a safe Rust model."
#[test]
fn divergence_safe_v2_registry_launders_a_platform_atomic_machine_door() {
    let temp = TempDir::new("v2-launder");
    let (output, bundle) = build_laundered_atomic(
        &temp,
        "thermite.frozen-primitive-registry.v2",
        Some("separate_verus_crate"),
        "separate_crate_verus_import",
        &[
            "contract_refinement",
            "exact_implementation_call",
            "exported_verus_interface",
            "imported_call_refinement",
            "separate_object_identity",
            "separate_source_identity",
            "whole_crate_no_cheating",
        ],
        "thermite_divergence_laundered_atomic_v2",
    );
    assert_machine_door_is_not_laundered("registry v2 separate_verus_crate", &output, &bundle);
}

/// Divergence (design-REQ miss): `.design/build/kernel-primitives.md`
/// §"Acceptance matrix" requires "Compose a synthetic test platform whose
/// bodies are tiny direct-Verus adapters", and
/// `.design/build/frozen-primitive-registry.md` §"Schema v1" makes sealed
/// consume/mint and borrow ownership part of every entry. The tracked registry
/// corpus exercises only `by_value` scalar `u64 -> u64` doors, so the
/// `consume_sealed` / `mint_sealed` / `shared_borrow` arms of
/// `primitive_registry.rs::parameter_ownership` and
/// `primitive_registry.rs::result_ownership` have no conformance fixture at
/// all — while most of the 74 tracked platform machine doors carry a sealed
/// parameter or result and 22 take a shared borrow.
#[test]
fn divergence_no_registry_fixture_exercises_the_sealed_ownership_transition() {
    // Authority: the tracked platform declaration surface.
    let api = fs::read_to_string(root().join("stdlib/kernel-primitives/platform/api.th")).unwrap();
    let parsed = thermite_syntax::parse(&api);
    assert!(parsed.is_clean(), "{:?}", parsed.errors);
    let sealed: BTreeSet<&str> = parsed
        .program
        .items
        .iter()
        .filter_map(|item| match item {
            thermite_syntax::Item::Struct(item) if item.sealed => Some(item.name.as_str()),
            _ => None,
        })
        .collect();
    let doors: Vec<_> = parsed
        .program
        .items
        .iter()
        .filter_map(|item| match item {
            thermite_syntax::Item::Fn(function) if function.boundary.is_some() => Some(function),
            _ => None,
        })
        .collect();
    // `.design/build/platform-primitives.md` §Scope: "74 exact bodyless
    // declarations remain L1 machine boundaries".
    assert_eq!(doors.len(), 74);
    let sealed_typed = doors
        .iter()
        .filter(|function| {
            function
                .params
                .iter()
                .any(|parameter| type_mentions(&parameter.ty, &sealed))
                || type_mentions(&function.ret, &sealed)
        })
        .count();
    let borrowed = doors
        .iter()
        .filter(|function| {
            function
                .params
                .iter()
                .any(|parameter| matches!(parameter.ty, thermite_syntax::Type::Ref { .. }))
        })
        .count();
    assert!(
        sealed_typed > 0 && borrowed > 0,
        "the platform door inventory must exercise sealed and borrowed ownership"
    );

    // Every tracked frozen-primitive registry fixture, identified by the
    // schema string `.design/build/frozen-primitive-registry.md` freezes.
    let mut fixtures = Vec::new();
    collect_registry_fixtures(&root().join("conformance"), &mut fixtures);
    fixtures.sort();
    assert!(
        !fixtures.is_empty(),
        "no tracked frozen-primitive registry fixture was found under conformance/"
    );

    let mut ownership_vocabulary = BTreeSet::new();
    for fixture in &fixtures {
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
        let empty = Vec::new();
        for entry in document["entries"].as_array().unwrap_or(&empty) {
            for parameter in entry["ownership"]["parameters"].as_array().unwrap_or(&empty) {
                ownership_vocabulary.insert(parameter.as_str().unwrap_or("?").to_string());
            }
            ownership_vocabulary.insert(
                entry["ownership"]["result"]
                    .as_str()
                    .unwrap_or("?")
                    .to_string(),
            );
        }
    }

    let relative: Vec<String> = fixtures
        .iter()
        .map(|path| {
            path.strip_prefix(root())
                .unwrap_or(path)
                .to_string_lossy()
                .to_string()
        })
        .collect();

    // `.design/build/frozen-primitive-registry.md` §"Schema v1": each entry
    // declares "parameter and result ownership (`by_value`, shared/exclusive
    // borrow, sealed consume, or sealed mint) derived independently from the
    // AST". The synthetic-platform acceptance bullet requires that these arms
    // are actually composed, not merely parseable.
    for required in ["shared_borrow", "consume_sealed", "mint_sealed"] {
        assert!(
            ownership_vocabulary.contains(required),
            "no tracked frozen-primitive registry fixture declares ownership `{required}`; \
             the corpus is {relative:?} with ownership vocabulary {ownership_vocabulary:?}. \
             `.design/build/kernel-primitives.md` §\"Acceptance matrix\" requires a synthetic \
             test platform of tiny direct-Verus adapters, and {sealed_typed} of the 74 tracked \
             platform machine doors carry a sealed parameter or result while {borrowed} take a \
             shared borrow — none of which any registry fixture composes."
        );
    }
}

fn type_mentions(ty: &thermite_syntax::Type, sealed: &BTreeSet<&str>) -> bool {
    use thermite_syntax::Type;
    match ty {
        Type::Named(name) => sealed.contains(name.as_str()),
        Type::Ref { inner, .. }
        | Type::Slice(inner)
        | Type::Array { elem: inner, .. }
        | Type::Generic { arg: inner, .. }
        | Type::Box(inner)
        | Type::Vec(inner)
        | Type::Option(inner) => type_mentions(inner, sealed),
        Type::Result(ok, err) | Type::Map(ok, err) => {
            type_mentions(ok, sealed) || type_mentions(err, sealed)
        }
        Type::Tuple(items) => items.iter().any(|item| type_mentions(item, sealed)),
        Type::Prim(_) | Type::Unit | Type::String => false,
    }
}

fn collect_registry_fixtures(directory: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_registry_fixtures(&path, out);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        if document["schema"]
            .as_str()
            .is_some_and(|schema| schema.starts_with(REGISTRY_SCHEMA_PREFIX))
        {
            out.push(path);
        }
    }
}
