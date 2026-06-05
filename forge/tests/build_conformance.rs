//! Conformance test for `forge build` (issue #56) against the EXTERNAL truth: the
//! real `rustc` compiler + the hand-derived oracle `conformance/build/cases.json`
//! (`.design/forge/build.md`). `forge build` lowers a Thermite program to
//! executable Rust (`thermite_lower::lower_l1`) and compiles it with rustc into a
//! contract-checked artifact whose always-active `thermite_check!`s fire at
//! RUNTIME (the #57 hook).
//!
//! Verification is by EXECUTION (the design's AC-1..AC-7): the artifact COMPILES
//! (rustc exit 0), the `--entry` binary RUNS and prints the hand-derived value, a
//! contract-violating body still COMPILES but its check FIRES at runtime, the
//! manifest records the per-fn `fx` rows, and the emitted source is byte-identical
//! across two builds. Expected values trace to the oracle / Appendix A's
//! `spec_sum` denotation (R-CHAR-3 — never copied from toolchain output).
//!
//! `rustc` is always installed (rustc 1.95.0; no skip). `unwrap`/`expect`/`panic!`
//! are fine here — `tests/` is not anti-pattern-gated. The `forge` binary is built
//! by cargo and invoked as a subprocess so the WHOLE CLI surface (`forge build
//! <file> [--entry <fn>] [--json]`) is exercised end-to-end.

use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// The compiled `forge` binary under test (cargo sets `CARGO_BIN_EXE_<name>`).
fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// Run `forge build <args...>` and return `(exit_success, stdout, stderr)`.
fn run_forge_build(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(forge_bin())
        .arg("build")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawning `forge build` failed: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Parse the `artifact:` path out of `forge build`'s `--json` document.
fn artifact_path_from_json(stdout: &str) -> PathBuf {
    let v: serde_json::Value = serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{stdout}"));
    let p = v["artifact"]
        .as_str()
        .unwrap_or_else(|| panic!("no `artifact` field in build manifest:\n{stdout}"));
    PathBuf::from(p)
}

/// Run a produced executable artifact and return `(exit_success, combined_output)`.
fn run_artifact(path: &Path) -> (bool, String) {
    let out = Command::new(path)
        .output()
        .unwrap_or_else(|e| panic!("running artifact `{}` failed: {e}", path.display()));
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), combined)
}

/// Write a throwaway `.th` fixture under the temp dir (cleaned by the caller).
fn write_fixture(name: &str, body: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("forge_build_test_{name}_{}.th", std::process::id()));
    std::fs::write(&path, body).unwrap_or_else(|e| panic!("write fixture {name}: {e}"));
    path
}

// ---- oracle `build_and_run` / AC-1, AC-3, AC-5 ------------------------------
//
// `forge build conformance/sum.th --entry sum` → rustc exit 0; running the binary
// prints output containing `6` (sum of [1,2,3], hand-derived from Appendix A's
// spec_sum); the manifest records sum's fx = ["pure"] (the #57 seccomp input).

#[test]
fn sum_runs() {
    let sum = corpus_dir().join("sum.th");
    let (ok, stdout, stderr) =
        run_forge_build(&[sum.to_str().unwrap(), "--entry", "sum", "--json"]);
    assert!(
        ok,
        "forge build sum.th --entry sum must succeed (rustc exit 0):\nstdout:{stdout}\nstderr:{stderr}"
    );

    // AC-5: the manifest's `sum` row carries fx = ["pure"] (Appendix A).
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("manifest JSON");
    let funcs = v["functions"].as_array().expect("functions array");
    let sum_row = funcs
        .iter()
        .find(|f| f["name"] == "sum")
        .expect("a `sum` function row");
    assert_eq!(
        sum_row["fx"],
        serde_json::json!(["pure"]),
        "AC-5: sum's fx row is [\"pure\"] (the #57 seccomp input):\n{stdout}"
    );
    assert_eq!(v["crate_type"], "bin", "an --entry build is a runnable bin");
    assert_eq!(v["entry"], "sum");

    // AC-3: the produced binary RUNS and prints the hand-derived value `6`.
    let artifact = artifact_path_from_json(&stdout);
    let (ran, output) = run_artifact(&artifact);
    assert!(
        ran,
        "the built sum binary must run clean (exit 0):\n{output}"
    );
    assert!(
        output.contains('6'),
        "AC-3: the built sum binary must print 6 (sum(&[1,2,3]) == 6, hand-derived):\n{output}"
    );

    let _ = std::fs::remove_file(&artifact);
    if let Some(parent) = artifact.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

// ---- AC-1 (library form) ----------------------------------------------------
//
// The default (no --entry) build is a compiled library (rlib) of the L1-checked
// fns; rustc exit 0 (dead-code warnings are allowed, a non-zero exit is a hard
// fail surfaced as ForgeError).

#[test]
fn sum_builds_as_library() {
    let sum = corpus_dir().join("sum.th");
    let (ok, stdout, stderr) = run_forge_build(&[sum.to_str().unwrap(), "--json"]);
    assert!(
        ok,
        "forge build sum.th (library) must succeed (rustc exit 0):\nstdout:{stdout}\nstderr:{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("manifest JSON");
    assert_eq!(v["crate_type"], "rlib", "the default artifact is a library");
    assert_eq!(
        v["assurance"], "L1 (built, runtime-checked)",
        "the honest L1 assurance statement (not a forged L3 claim)"
    );
    let artifact = artifact_path_from_json(&stdout);
    assert!(
        artifact.exists() && artifact.extension().map(|e| e == "rlib").unwrap_or(false),
        "the rlib artifact must exist at the reported path: {}",
        artifact.display()
    );
    let _ = std::fs::remove_file(&artifact);
    if let Some(parent) = artifact.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

// ---- oracle `runtime_violation` / AC-4 (the #57 kill behavior) --------------
//
// `bad(x) req x < 100 ens result == x { x + 1 }`: forge build (L1) COMPILES it
// (rustc exit 0 — L1 does not verify, it checks at runtime); the runner calls
// bad(<sample x<100>) → the always-active ens check FIRES at runtime (non-zero
// exit + an [ens] contract-violation diagnostic).

#[test]
fn ens_violation_fires_at_runtime() {
    let prog =
        "fn bad(x: u32) -> u32\n  req x < 100\n  ens result == x\n  fx  pure\n{\n  x + 1\n}\n";
    let fixture = write_fixture("bad", prog);
    let (ok, stdout, stderr) =
        run_forge_build(&[fixture.to_str().unwrap(), "--entry", "bad", "--json"]);
    assert!(
        ok,
        "forge build (L1) must COMPILE the contract-violating `bad` (rustc exit 0):\nstdout:{stdout}\nstderr:{stderr}"
    );
    let artifact = artifact_path_from_json(&stdout);

    // Running it: the always-active ens check FIRES — non-zero exit + an [ens]
    // diagnostic (the runtime-enforcement behavior #57 builds on).
    let (ran, output) = run_artifact(&artifact);
    assert!(
        !ran,
        "the built `bad` binary must ABORT at the violated ens check (non-zero exit):\n{output}"
    );
    assert!(
        output.contains("thermite L1 contract violation [ens]")
            || output.contains("contract violation [ens]")
            || output.contains("ens"),
        "AC-4: the runtime ens check must fire with an [ens] diagnostic:\n{output}"
    );

    let _ = std::fs::remove_file(&artifact);
    if let Some(parent) = artifact.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
    let _ = std::fs::remove_file(&fixture);
}

// ---- AC-2: checks are baked in (always-active, NOT debug_assert) -------------
//
// The compiled artifact is `thermite_lower::lower_l1`'s output verbatim (build.rs
// never strips it). The §6 every-profile property — the always-active
// `thermite_check!` macro (`if !($cond)`, NOT debug_assert) — is structurally
// present in that emission. Anchored to the public `lower_l1` (the EXACT bytes
// build_file compiles), the same property `l1_conformance.rs::
// no_debug_assert_in_emission` pins (R-CHAR-3 — the §6 design property, not
// toolchain self-equality).

#[test]
fn checks_are_baked_in() {
    let sum = corpus_dir().join("sum.th");
    let src = lower_corpus_l1(&sum);
    assert!(
        src.contains("macro_rules! thermite_check"),
        "AC-2: the compiled source must define the always-active thermite_check macro:\n{src}"
    );
    assert!(
        src.contains("if !($cond)"),
        "AC-2: the macro must be the always-active `if !(cond)` form:\n{src}"
    );
    assert!(
        !src.contains("debug_assert"),
        "AC-2: the compiled source must NOT use debug_assert (stripped in release; §6):\n{src}"
    );
}

// ---- AC-6: reproducible artifact (deterministic source + pinned codegen) -----
//
// Two same-input `forge build` runs (same toolchain, SOURCE_DATE_EPOCH=0 pinned,
// the per-run scratch path remapped out of the debug metadata) produce a
// BYTE-IDENTICAL compiled rlib (§5.3). The emitted source is forge-owned
// deterministic; the codegen is reproducible modulo nothing once the path + the
// archive mtime are pinned. This builds via the real CLI twice and diffs the
// actual artifact bytes (R-CHAR-3 — the design's reproducibility AC, not a
// toolchain self-comparison of derived strings).

#[test]
fn rebuilt_library_is_byte_identical() {
    let sum = corpus_dir().join("sum.th");
    let sum = sum.to_str().unwrap();

    let (ok_a, out_a, err_a) = run_forge_build(&[sum, "--json"]);
    let (ok_b, out_b, err_b) = run_forge_build(&[sum, "--json"]);
    assert!(ok_a && ok_b, "both builds must succeed:\n{err_a}\n{err_b}");
    let art_a = artifact_path_from_json(&out_a);
    let art_b = artifact_path_from_json(&out_b);

    let bytes_a = std::fs::read(&art_a).expect("read artifact a");
    let bytes_b = std::fs::read(&art_b).expect("read artifact b");
    assert_eq!(
        bytes_a, bytes_b,
        "AC-6: two same-input rlib builds must be byte-identical (SOURCE_DATE_EPOCH + path \
         pinned; §5.3 reproducibility)"
    );

    for art in [&art_a, &art_b] {
        let _ = std::fs::remove_file(art);
        if let Some(parent) = art.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

// ---- AC-7: exit-status discipline (R-CODE-4) --------------------------------
//
// A program that parses/validates/effect-checks clean but lowers to Rust rustc
// REJECTS (an `ens` referencing an undefined identifier) yields a non-zero `forge
// build` exit and a structured RustcOutput error — never a silent success.

#[test]
fn uncompilable_lowering_is_nonzero_exit() {
    let prog = "fn f(x: u32) -> u32\n  req x < 100\n  ens result == nonexistent_thing\n  fx  pure\n{\n  x\n}\n";
    let fixture = write_fixture("uncompilable", prog);
    let (ok, stdout, stderr) = run_forge_build(&[fixture.to_str().unwrap(), "--json"]);
    assert!(
        !ok,
        "AC-7: forge build must FAIL (non-zero) when rustc rejects the lowered source:\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        stderr.contains("rustc failed") || stderr.contains("nonexistent_thing"),
        "AC-7: the failure must be a structured rustc error, never swallowed:\nstderr:{stderr}"
    );
    let _ = std::fs::remove_file(&fixture);
}

// ---- helper: the EXACT bytes build_file compiles for the library form --------
//
// `build_file` compiles `thermite_lower::lower_l1(program)` verbatim (build.rs's
// `emit_source` is `lower_l1` + an optional appended runner). AC-2 inspects that
// emission directly through the public `lower_l1` (the same bytes the artifact is
// compiled from) — anchored to the §6 design property, not a self-comparison
// (R-CHAR-3).

fn lower_corpus_l1(path: &Path) -> String {
    let src =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let parsed = thermite_syntax::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "fixture must parse clean: {:?}",
        parsed.errors
    );
    thermite_lower::lower_l1(&parsed.program).expect("lower_l1")
}
