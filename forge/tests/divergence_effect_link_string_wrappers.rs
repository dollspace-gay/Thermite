//! DIVERGENCE PIN (acto-critic, Basis Stage 8 / issue #81): the Write/Read-line
//! `os::<name>` wrappers the design ENUMERATES do not LINK + RUN.
//!
//! Authority: `.design/basis/08-runnable-effect-link.md`
//!   - REQ-1: "`thermite-stdlib/src/effect/{read,write,time}.rs` provide a real
//!     Rust syscall wrapper `fn` for each v1 `os::<name>` target: … `os::read_line`
//!     (… the latter over Stage 7 `String`), `os::write`/`os::print`
//!     (`std::io::stdout().write_all`, Stage 7 `String` arg). Each wrapper's
//!     signature MATCHES the `#[boundary]` primitive it backs (params + return)."
//!   - REQ-3: "a verified program using an effect primitive COMPILES + RUNS + does
//!     real I/O".
//!   - the wrapper-set table: rows `os::write` / `os::print` (`write(output)`,
//!     Stage-7 `String` arg) + `os::read_line` (Stage-7 `String` return).
//!   - REQ status table marks REQ-1 SHIPPED with `os::write`/`os::print` "over
//!     `TString`" and `os::read_line` "→ `TString`".
//!
//! THE DIVERGENCE: `forge/src/effect_wrappers.rs` `WRAPPERS` emits the
//! `os::write`/`os::print`/`os::read_line` bodies referencing `super::TString`,
//! and `thermite_lower::lower_l1` lowers a `String`-typed boundary fn's signature
//! to the bare type name `TString` (`thermite-lower/src/l1.rs` `lower_type` arm
//! `Type::String => Ok("TString")`). But neither `emit_mod_os` nor `lower_l1`
//! EMITS a `struct TString` definition into the BUILD-emitted crate (the `TString`
//! struct lives only in the L3/Verus lowering, `thermite-lower/src/lower.rs`). So
//! `forge build` of ANY program using `os::write`/`os::print`/`os::read_line`
//! `rustc`-FAILS `error[E0425]: cannot find type \`TString\``.
//!
//! Three of the five wrappers REQ-1 enumerates (Write family + read_line) therefore
//! do NOT "COMPILE + RUN + do real I/O" — REQ-1 / REQ-3 are NOT-STARTED for the
//! Write/read_line families, contradicting the REQ-status "SHIPPED" claim.
//!
//! Reproduced live: `forge build print_demo.th --entry greet` →
//!   `error[E0425]: cannot find type \`TString\` in module \`super\``
//!   `error[E0425]: cannot find type \`TString\` in this scope`
//!
//! Tracking: blocker #N (filed by the critic; un-ignore when the build emits a
//! `struct TString` / lowers `String` to a build-resolvable type).

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// Run `forge build <args...>` → `(exit_success, stdout, stderr)`.
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

fn artifact_path_from_json(stdout: &str) -> Option<PathBuf> {
    let v: Value = serde_json::from_str(stdout).ok()?;
    v["artifact"].as_str().map(PathBuf::from)
}

fn cleanup(artifact: &std::path::Path) {
    let _ = std::fs::remove_file(artifact);
    if let Some(parent) = artifact.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

/// The design's Write-family wrapper: a `#[boundary("os::print")]` primitive over a
/// Stage-7 `String` arg (`08-runnable-effect-link.md` REQ-1 / the wrapper-set
/// table). Hand-derived from the doc, NOT copied from toolchain output (R-CHAR-3).
const PRINT_DEMO: &str = "#[boundary(\"os::print\")] fn print(s: String) -> u64\n  \
                          req true\n  ens result <= 1\n  fx  write(output)\n  ;\n\n\
                          fn greet() -> u64\n  req true\n  ens result <= 1\n  \
                          fx  write(output)\n{\n  print(String::new())\n}\n";

/// The design's read_line wrapper: a `#[boundary("os::read_line")]` primitive
/// returning a Stage-7 `String` (`08-runnable-effect-link.md` REQ-1).
const READ_LINE_DEMO: &str = "#[boundary(\"os::read_line\")] fn read_line() -> String\n  \
                              req true\n  ens true\n  fx  read(input)\n  ;\n\n\
                              fn getit() -> String\n  req true\n  ens true\n  \
                              fx  read(input)\n{\n  read_line()\n}\n";

fn write_fixture(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "divergence_effect_link_{name}_{}.th",
        std::process::id()
    ));
    std::fs::write(&p, src).expect("write fixture");
    p
}

#[test]
#[ignore = "divergence: os::print/os::write wrapper references undefined `super::TString`; \
            the build-emitted crate defines no `struct TString`, so forge build of a Write-family \
            boundary fails E0425 (08-runnable-effect-link.md REQ-1/REQ-3); blocker #N — un-ignore when fixed"]
fn print_wrapper_builds_and_runs() {
    // AUTHORITY (08-runnable-effect-link.md REQ-1): "os::write/os::print
    // (std::io::stdout().write_all, Stage 7 String arg). Each wrapper's signature
    // MATCHES the #[boundary] primitive it backs"; REQ-3: it COMPILES + RUNS.
    let fixture = write_fixture("print", PRINT_DEMO);
    let (ok, stdout, stderr) =
        run_forge_build(&[fixture.to_str().unwrap(), "--entry", "greet", "--json"]);
    let _ = std::fs::remove_file(&fixture);

    // The link is broken: the emitted `pub fn print(s: super::TString)` (and the
    // lowered `fn print(s: TString)`) reference an undefined `TString`.
    assert!(
        !stderr.contains("cannot find type `TString`")
            && !stdout.contains("cannot find type `TString`"),
        "REQ-1/REQ-3: the os::print Write wrapper must LINK (no undefined `TString`); \
         got:\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        ok,
        "REQ-3: forge build --entry greet (an os::print Write primitive) must COMPILE \
         (rustc exit 0):\nstdout:{stdout}\nstderr:{stderr}"
    );
    if let Some(artifact) = artifact_path_from_json(&stdout) {
        cleanup(&artifact);
    }
}

#[test]
#[ignore = "divergence: os::read_line wrapper references undefined `super::TString`; \
            forge build of a read_line boundary fails E0425/E0422 (08-runnable-effect-link.md \
            REQ-1/REQ-3); blocker #N — un-ignore when fixed"]
fn read_line_wrapper_builds() {
    // AUTHORITY (08-runnable-effect-link.md REQ-1): "os::read_byte/os::read_line
    // (std::io::stdin().read/read_line, the latter over Stage 7 String)".
    let fixture = write_fixture("read_line", READ_LINE_DEMO);
    let (ok, stdout, stderr) =
        run_forge_build(&[fixture.to_str().unwrap(), "--entry", "getit", "--json"]);
    let _ = std::fs::remove_file(&fixture);

    assert!(
        !stderr.contains("cannot find type `TString`")
            && !stderr.contains("cannot find struct, variant or union type `TString`"),
        "REQ-1/REQ-3: the os::read_line wrapper must LINK (no undefined `TString`); \
         got:\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        ok,
        "REQ-3: forge build --entry getit (an os::read_line primitive) must COMPILE:\n\
         stdout:{stdout}\nstderr:{stderr}"
    );
    if let Some(artifact) = artifact_path_from_json(&stdout) {
        cleanup(&artifact);
    }
}
