//! `forge/tests/divergence_package_commands.rs` — pinned divergence for
//! REQ-KPRIM-3 (`.design/build/kernel-primitives.md`, section "Modules,
//! packages, and receipts").
//!
//! AUTHORITY. `.design/build/kernel-primitives.md` states the package primitive
//! must provide "one canonical package manifest with package identity and
//! explicit roots", "relative module imports with no ambient search path", and
//! "a complete transitive `.th` source closure", and that "Package support must
//! not be implemented by concatenating files without preserving source identity
//! and diagnostic spans." Its residual scope in `.design/reqs/registry.toml`
//! (`REQ-KPRIM-3`) reads verbatim: "Extend the remaining source-oriented Forge
//! commands (check, audit, TV, goal/edit/fill) to operate on packages without
//! losing module-local diagnostics."
//!
//! DIVERGENCE. Only `forge build --level l3` / `forge verify-build` route through
//! `thermite_package::load` (`forge/src/verified_build.rs`). Every command named
//! in the residual scope reads its argument with `std::fs::read_to_string` and
//! hands the bytes straight to `thermite_syntax::parse` — see `fn check_file` in
//! `forge/src/check.rs`, `fn run_audit` in `forge/src/cli.rs`, `fn tv_file` in
//! `forge/src/contract_tv.rs`, `fn exec_tv_file` in `forge/src/exec_tv.rs`,
//! `fn body_tv_file` in `forge/src/body_tv.rs`, and `fn parse_program` /
//! `fn edit_file` / `fn fill_file` in `forge/src/goal_repl.rs`. Pointed at a
//! `.thpkg.json` manifest they therefore lex canonical JSON as Thermite source
//! and die on its opening brace.
//!
//! R-CHAR-3. Every expected value below is derived from the design doc or read
//! at test time out of the real package manifests / module sources under
//! `stdlib/kernel-primitives/`. The one literal quoted from a forge run —
//! [`MANIFEST_LEXED_AS_SOURCE`] — names the WRONG behaviour the tests forbid; it
//! is never used as an expected value.
//!
//! Tracking: see the `-l blocker` crosslink issue referenced in each test.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// The literal diagnostic forge emits when a source-oriented command lexes a
/// canonical `.thpkg.json` manifest as Thermite source. `{` is the manifest's
/// first byte, so this string appearing in a command's output is mechanical
/// proof that the command never routed through `thermite_package::load`.
const MANIFEST_LEXED_AS_SOURCE: &str = "found `{` at byte 0";

static TEST_NONCE: AtomicU64 = AtomicU64::new(0);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn stdlib() -> PathBuf {
    repo_root().join("stdlib/kernel-primitives")
}

/// The smaller real package: one root module `generation`, no imports.
fn ownership_manifest() -> PathBuf {
    stdlib().join("ownership.thpkg.json")
}

/// The only real package with a transitive import closure: roots `api` and
/// `atomic_storage` over six modules.
fn atomics_manifest() -> PathBuf {
    stdlib().join("atomics.thpkg.json")
}

/// The five-root collection package.
fn collections_manifest() -> PathBuf {
    stdlib().join("collections.thpkg.json")
}

/// Run the real `forge` binary and return `(exit code, stdout ++ stderr)`.
fn forge(args: &[&str]) -> (Option<i32>, String) {
    let out = Command::new(forge_bin())
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("spawn forge");
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.code(), combined)
}

/// The shared acceptance assertion for REQ-KPRIM-3's residual scope: a
/// source-oriented command pointed at a canonical manifest must not treat the
/// manifest bytes as Thermite source.
fn assert_accepts_manifest(verb: &str, args: &[&str], combined: &str) {
    assert!(
        !combined.contains(MANIFEST_LEXED_AS_SOURCE),
        "`forge {verb}` lexed the canonical package manifest as Thermite source \
         instead of loading it as a package.\n\
         .design/build/kernel-primitives.md (Modules, packages, and receipts) requires \
         `{verb}` to consume `one canonical package manifest with package identity and \
         explicit roots`; REQ-KPRIM-3's residual scope names it explicitly.\n\
         argv: {args:?}\n\
         output:\n{combined}"
    );
}

fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn read_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The declared module paths of a canonical manifest, in manifest order. Read
/// out of the real manifest so the fixtures below are grounded in the package's
/// own structure rather than in a hand-copied list (R-CHAR-3).
fn declared_module_paths(manifest_text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut rest = manifest_text;
    while let Some(at) = rest.find("\"path\": \"") {
        rest = &rest[at + "\"path\": \"".len()..];
        let end = rest.find('"').expect("terminated manifest path string");
        paths.push(rest[..end].to_string());
        rest = &rest[end..];
    }
    paths
}

/// Every `at byte N` offset a forge diagnostic reports.
fn reported_byte_offsets(text: &str) -> Vec<u64> {
    let mut offsets = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("at byte ") {
        rest = &rest[at + "at byte ".len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(value) = digits.parse::<u64>() {
            offsets.push(value);
        }
    }
    offsets
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "thermite-divergence-package-{tag}-{}-{}",
        std::process::id(),
        TEST_NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Copy a real package (its manifest plus every module the manifest declares)
/// into `dest`, preserving the declared relative paths. Returns the copied
/// manifest path.
fn copy_package(manifest: &Path, dest: &Path) -> PathBuf {
    let manifest_text = read_text(manifest);
    let source_root = manifest.parent().expect("manifest has a parent directory");
    for relative in declared_module_paths(&manifest_text) {
        let from = source_root.join(&relative);
        let to = dest.join(&relative);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).expect("create module directory");
        }
        std::fs::copy(&from, &to)
            .unwrap_or_else(|e| panic!("copy {} -> {}: {e}", from.display(), to.display()));
    }
    let name = manifest
        .file_name()
        .expect("manifest has a file name")
        .to_string_lossy()
        .into_owned();
    let copied = dest.join(name);
    std::fs::write(&copied, manifest_text.as_bytes()).expect("write manifest copy");
    copied
}

// ---------------------------------------------------------------------------
// Residual scope, command by command. `.design/reqs/registry.toml` REQ-KPRIM-3:
// "Extend the remaining source-oriented Forge commands (check, audit, TV,
//  goal/edit/fill) to operate on packages without losing module-local
//  diagnostics."
// ---------------------------------------------------------------------------

#[test]
fn check_accepts_a_package_manifest() {
    let manifest = ownership_manifest();
    let path = manifest.to_string_lossy().into_owned();
    let args = ["check", path.as_str(), "--json"];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("check", &args, &combined);
}

#[test]
fn audit_accepts_a_package_manifest() {
    let manifest = ownership_manifest();
    let path = manifest.to_string_lossy().into_owned();
    let args = ["audit", path.as_str(), "--json"];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("audit", &args, &combined);
}

#[test]
fn contract_tv_accepts_a_package_manifest() {
    let manifest = ownership_manifest();
    let path = manifest.to_string_lossy().into_owned();
    let args = ["tv", path.as_str(), "--json"];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("tv", &args, &combined);
}

#[test]
fn exec_tv_accepts_a_package_manifest() {
    let manifest = ownership_manifest();
    let path = manifest.to_string_lossy().into_owned();
    let args = ["exec-tv", path.as_str(), "--json"];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("exec-tv", &args, &combined);
}

#[test]
fn body_tv_accepts_a_package_manifest() {
    let manifest = ownership_manifest();
    let path = manifest.to_string_lossy().into_owned();
    let args = ["body-tv", path.as_str(), "--json"];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("body-tv", &args, &combined);
}

#[test]
fn goal_accepts_a_package_manifest() {
    let manifest = ownership_manifest();
    let path = manifest.to_string_lossy().into_owned();
    // `generation_ledger_init` is a real item of the package's only module,
    // `ownership/generation.th` (asserted from the source below, not from a
    // forge run).
    let module = stdlib().join("ownership/generation.th");
    let source = read_text(&module);
    assert!(
        source.contains("fn generation_ledger_init("),
        "fixture drift: `ownership/generation.th` no longer declares \
         `generation_ledger_init`"
    );
    let args = ["goal", path.as_str(), "generation_ledger_init"];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("goal", &args, &combined);
}

#[test]
fn fill_accepts_a_package_manifest() {
    let manifest = collections_manifest();
    let path = manifest.to_string_lossy().into_owned();
    // `fixed_bitmap_count` is declared in the package's root module `bitmap`
    // (`collections/bitmap.th`).
    let module = stdlib().join("collections/bitmap.th");
    let source = read_text(&module);
    assert!(
        source.contains("fn fixed_bitmap_count("),
        "fixture drift: `collections/bitmap.th` no longer declares `fixed_bitmap_count`"
    );
    let args = ["fill", path.as_str(), "fixed_bitmap_count.?0", "0"];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("fill", &args, &combined);
}

// ---------------------------------------------------------------------------
// The two properties the residual scope names beyond "accepts a manifest":
// the complete transitive closure, and module-local diagnostics.
// ---------------------------------------------------------------------------

/// `.design/build/kernel-primitives.md`: the package primitive supplies "a
/// complete transitive `.th` source closure" and "relative module imports with
/// no ambient search path". `atomics.thpkg.json` declares root `api` importing
/// `init`, `machine`, and `model`; `src/model.th` declares
/// `spec fn atomic_order_code` and `const ATOMIC_HISTORY_CAPACITY`. A
/// source-oriented command pointed at that manifest must therefore see those
/// declarations — reporting them undeclared proves the closure was never
/// resolved.
#[test]
fn goal_resolves_the_transitive_module_closure() {
    let manifest = atomics_manifest();
    let manifest_text = read_text(&manifest);
    assert!(
        manifest_text.contains("\"name\": \"api\"") && manifest_text.contains("\"model\""),
        "fixture drift: `atomics.thpkg.json` no longer declares root `api` importing `model`"
    );
    let model = read_text(&stdlib().join("src/model.th"));
    assert!(
        model.contains("spec fn atomic_order_code(")
            && model.contains("const ATOMIC_HISTORY_CAPACITY: usize ="),
        "fixture drift: `src/model.th` no longer declares `atomic_order_code` / \
         `ATOMIC_HISTORY_CAPACITY`"
    );

    let path = manifest.to_string_lossy().into_owned();
    let args = ["goal", path.as_str()];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("goal", &args, &combined);
    assert!(
        !combined.contains("`atomic_order_code` is not a registered SpecTherm combinator"),
        "`forge goal` did not resolve the package's transitive module closure: \
         `atomic_order_code` is declared as a `spec fn` in module `model`, which root \
         `api` imports in `atomics.thpkg.json`.\n\
         Authority: .design/build/kernel-primitives.md — `a complete transitive `.th` \
         source closure`.\n\
         output:\n{combined}"
    );
    assert!(
        !combined.contains("array capacity `ATOMIC_HISTORY_CAPACITY` is not a declared"),
        "`forge goal` did not resolve the package's transitive module closure: \
         `ATOMIC_HISTORY_CAPACITY` is declared in module `model`, which root `api` imports \
         in `atomics.thpkg.json`.\n\
         output:\n{combined}"
    );
}

/// `.design/build/kernel-primitives.md`: "Package support must not be
/// implemented by concatenating files without preserving source identity and
/// diagnostic spans." `forge edit` resolves a semantic address and splices the
/// replacement at its span, so on a package it must rewrite the module that
/// declares the addressed item and leave the manifest and every sibling module
/// byte-identical.
///
/// The replacement `@@@` is deliberately unparseable: forge writes the splice
/// and then reports the re-parse failure, which keeps this test off the verus
/// path in both the current and the fixed toolchain.
#[test]
fn edit_splices_into_the_declaring_module_not_the_manifest() {
    let dir = scratch_dir("edit");
    let manifest = copy_package(&collections_manifest(), &dir);
    let manifest_before = read_bytes(&manifest);
    let module_paths = declared_module_paths(&read_text(&manifest));
    assert_eq!(
        module_paths.len(),
        5,
        "fixture drift: `collections.thpkg.json` no longer declares five modules"
    );
    let before: Vec<Vec<u8>> = module_paths
        .iter()
        .map(|relative| read_bytes(&dir.join(relative)))
        .collect();

    let path = manifest.to_string_lossy().into_owned();
    let args = [
        "edit",
        path.as_str(),
        "fixed_bitmap_count.loop#1.inv#1",
        "--replace",
        "@@@",
    ];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("edit", &args, &combined);

    assert_eq!(
        read_bytes(&manifest),
        manifest_before,
        "`forge edit` rewrote the canonical package manifest instead of the module that \
         declares `fixed_bitmap_count`.\n\
         Authority: .design/build/kernel-primitives.md — package support `must not be \
         implemented by concatenating files without preserving source identity`."
    );

    let declaring = "collections/bitmap.th";
    let after = read_text(&dir.join(declaring));
    assert!(
        after.contains("inv @@@"),
        "`forge edit` did not splice the replacement into `{declaring}`, the module that \
         declares `fixed_bitmap_count` in `collections.thpkg.json`.\n\
         Authority: .design/build/kernel-primitives.md — source identity and diagnostic \
         spans are preserved across the package.\n\
         output:\n{combined}"
    );
    for (relative, original) in module_paths.iter().zip(before.iter()) {
        if relative == declaring {
            continue;
        }
        assert_eq!(
            &read_bytes(&dir.join(relative)),
            original,
            "`forge edit` modified sibling module `{relative}`, which does not declare \
             `fixed_bitmap_count`"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// REQ-KPRIM-3: the source-oriented commands operate on packages "without
/// losing module-local diagnostics". `thermite_syntax::PackageParseError`
/// already renders `module `<name>` (<path>): <error>` over a span that is
/// documented as "a span into this origin's module source, never a package-wide
/// offset" (`ItemOrigin` in `thermite-syntax/src/package.rs`). A `forge check`
/// on a package whose fourth module is corrupted must therefore name that
/// module's path and report an offset inside that module — not an offset into
/// the concatenated backend projection.
#[test]
fn check_package_diagnostics_keep_module_local_identity() {
    let dir = scratch_dir("diag");
    let manifest = copy_package(&collections_manifest(), &dir);
    let corrupted = "collections/ring.th";
    let module_path = dir.join(corrupted);
    let mut source = read_text(&module_path);
    source.push_str("\nfn\n");
    std::fs::write(&module_path, source.as_bytes()).expect("write corrupted module");
    let module_len = source.len() as u64;

    let path = manifest.to_string_lossy().into_owned();
    let args = ["check", path.as_str()];
    let (_code, combined) = forge(&args);
    assert_accepts_manifest("check", &args, &combined);

    assert!(
        combined.contains(corrupted),
        "`forge check` lost module-local identity: the diagnostic for the corrupted module \
         never names its package-relative path `{corrupted}`.\n\
         Authority: .design/reqs/registry.toml REQ-KPRIM-3 — `without losing module-local \
         diagnostics`.\n\
         output:\n{combined}"
    );
    let offsets = reported_byte_offsets(&combined);
    assert!(
        !offsets.is_empty(),
        "`forge check` reported no byte offset for the corrupted module.\n\
         output:\n{combined}"
    );
    for offset in offsets {
        assert!(
            offset <= module_len,
            "`forge check` reported byte offset {offset}, which is outside \
             `{corrupted}` ({module_len} bytes) — a concatenated projection offset, not the \
             in-module span.\n\
             Authority: thermite-syntax `ItemOrigin` — `a span into this origin's module \
             source, never a package-wide offset`.\n\
             output:\n{combined}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
