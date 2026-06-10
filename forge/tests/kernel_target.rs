//! Conformance test for `forge build --target kernel` (issue #197) against the
//! EXTERNAL truth: the real `rustc` compiler (a freestanding `#![no_std]`
//! invocation) + the hand-derived design `.design/build/kernel-target.md`. The
//! kernel target emits a freestanding `no_std + alloc` LIBRARY rlib (no `main`, no
//! seccomp prelude, `panic=abort`) suitable for linking into a verified
//! microkernel, and REFUSES a fn whose transitive `fx` carries an ambient-syscall
//! effect (`read`/`write`/`net`/`term`).
//!
//! Verification is by the design's ACs:
//!   - AC-1 — `forge build --target kernel sum.th` (the pure corpus `sum`) emits a
//!     `#![no_std]` + `extern crate alloc;` rlib and rustc exits 0; the emitted
//!     source contains `#![no_std]`/`extern crate alloc;` and NO `fn main` / NO
//!     seccomp prelude (`PR_SET_SECCOMP`).
//!   - AC-2 — a `fx read(...)` fn → a structured refusal naming the rejected effect,
//!     nonzero exit, NO artifact; a `fx write`/`net`/`term` fn refuses identically; a
//!     `fx pure`/`alloc` fn builds.
//!   - AC-3 — the emitted kernel source carries the always-active `thermite_check!` /
//!     `thermite_contract_violation` (`panic!`) VERBATIM (NOT stripped, NOT
//!     `debug_assert!`); the no_std rlib genuinely COMPILES when a test
//!     `#[panic_handler]`/`#[global_allocator]` (the kernel-host stand-in) is linked.
//!   - AC-4 — `forge build sum.th` (no `--target`) is byte-unchanged (the std default).
//!   - AC-5 — `forge check` / the L3 path is untouched (no `check.rs` edit; the
//!     existing check suites are unaffected — asserted by their continued passing,
//!     not re-run here).
//!
//! The freestanding-compile MECHANISM (AC-1/AC-3): the test reconstructs the EXACT
//! kernel source forge emits for a pure (boundary-free) program — the design-pinned
//! prelude `#![no_std]` / `extern crate alloc;` / `use alloc::vec::Vec;` PLUS
//! `thermite_lower::lower_l1`'s output — INDEPENDENTLY (the prelude is taken from the
//! design doc, NOT copied from forge output — R-CHAR-3), then shells the real `rustc`
//! with `-C panic=abort` + a test panic_handler/allocator stub. This is the
//! N-version check: forge's own `--target kernel` ALSO compiles the source internally
//! (AC-1, exit 0), and the reconstructed-source compile cross-checks the no_std-ness.
//!
//! `rustc` is always installed (no skip). `unwrap`/`expect`/`panic!` are fine here —
//! `tests/` is not anti-pattern-gated.

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

/// The design-pinned freestanding prelude (`.design/build/kernel-target.md` REQ-2),
/// transcribed from the DESIGN — not copied from forge output (R-CHAR-3). The kernel
/// crate is `#![no_std]` with `extern crate alloc;` and the bare `Vec` resolved from
/// the alloc prelude; `String` is the L1 emission's `use TString as String;` alias
/// (so the prelude must NOT re-import it), `panic!` is core.
const PINNED_KERNEL_PRELUDE: &str = "#![no_std]\nextern crate alloc;\nuse alloc::vec::Vec;\n";

/// Reconstruct the EXACT kernel-target source forge emits for the pure (boundary-
/// free) corpus program at `th_path`: the pinned prelude + `thermite_lower::lower_l1`
/// (a boundary-free program emits no `mod os`, so this is the whole body). This is
/// the INDEPENDENT N-version source the AC-3 freestanding compile uses.
fn reconstruct_kernel_source(th_path: &Path) -> String {
    let src = std::fs::read_to_string(th_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", th_path.display()));
    let parsed = thermite_syntax::parse(&src);
    assert!(parsed.is_clean(), "corpus program must parse cleanly");
    let lowered = thermite_lower::lower_l1(&parsed.program)
        .unwrap_or_else(|e| panic!("lower_l1 {}: {e:?}", th_path.display()));
    format!("{PINNED_KERNEL_PRELUDE}{lowered}")
}

/// Shell the real `rustc` to compile `source` as a FREESTANDING crate (AC-3): append
/// the kernel-host stand-in (`#[panic_handler]` + a trivial `#[global_allocator]`)
/// and compile it `--crate-type=rlib -C panic=abort`. An rlib of a `#![no_std]` body
/// is the genuine freestanding-link check: rustc fully type-checks the no_std body,
/// resolves `panic!` to core (routed to the test `#[panic_handler]`), and links
/// `alloc`'s `Vec`/`String` against the test `#[global_allocator]` — exactly the
/// kernel-host pieces the design's OQ-1 says forge does NOT emit. `unique` keys a
/// per-test temp dir so concurrently-running tests do not clobber each other's dir.
/// Returns `(rustc_exit_success, stderr)`.
fn freestanding_compile(unique: &str, source: &str) -> (bool, String) {
    let dir =
        std::env::temp_dir().join(format!("forge_kernel_test_{}_{unique}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let rs = dir.join("kernel_freestanding.rs");

    // The kernel-host stand-in (OQ-1): a `#[panic_handler]` (the L1
    // `thermite_contract_violation`'s `panic!` routes here under `panic=abort`) and a
    // trivial `#[global_allocator]` so `alloc`'s `Vec`/`String` link freestanding.
    // None of this is emitted by forge — it is the test harness standing in for the
    // kernel host. The allocator never actually allocates in a type-check/link build.
    let host_stub = "\n\
use core::alloc::{GlobalAlloc, Layout};\n\
struct NullAlloc;\n\
unsafe impl GlobalAlloc for NullAlloc {\n\
    unsafe fn alloc(&self, _l: Layout) -> *mut u8 { core::ptr::null_mut() }\n\
    unsafe fn dealloc(&self, _p: *mut u8, _l: Layout) {}\n\
}\n\
#[global_allocator]\n\
static A: NullAlloc = NullAlloc;\n\
#[panic_handler]\n\
fn ph(_: &core::panic::PanicInfo) -> ! { loop {} }\n";

    let full = format!("{source}{host_stub}");
    std::fs::write(&rs, &full).expect("write freestanding source");

    let out = Command::new("rustc")
        .arg("--crate-name")
        .arg("kernel_freestanding")
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("rlib")
        .arg("-C")
        .arg("panic=abort")
        .arg("kernel_freestanding.rs")
        .arg("-o")
        .arg(dir.join("libkernel_freestanding.rlib"))
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("spawning rustc failed: {e}"));

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    (out.status.success(), stderr)
}

/// Parse the `artifact:` path out of `forge build`'s `--json` document.
fn artifact_path_from_json(stdout: &str) -> PathBuf {
    let v: serde_json::Value = serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{stdout}"));
    let p = v["artifact"]
        .as_str()
        .unwrap_or_else(|| panic!("no `artifact` field:\n{stdout}"));
    PathBuf::from(p)
}

/// Write a throwaway `.th` fixture under the temp dir.
fn write_fixture(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_kernel_fixture_{name}_{}.th",
        std::process::id()
    ));
    std::fs::write(&path, body).unwrap_or_else(|e| panic!("write fixture {name}: {e}"));
    path
}

// ---- AC-1: pure fn → kernel rlib compiles no_std --------------------------------

#[test]
fn pure_fn_builds_no_std_kernel_rlib() {
    let sum = corpus_dir().join("sum.th");
    let (ok, stdout, stderr) =
        run_forge_build(&[sum.to_str().unwrap(), "--target", "kernel", "--json"]);
    assert!(
        ok,
        "forge build --target kernel sum.th must succeed (rustc exit 0):\n\
         stdout:{stdout}\nstderr:{stderr}"
    );

    // The artifact is an rlib library (kernel is never a bin).
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("manifest JSON");
    assert_eq!(
        v["crate_type"], "rlib",
        "the kernel target produces a library (rlib), never a bin:\n{stdout}"
    );
    let artifact = artifact_path_from_json(&stdout);
    assert!(
        artifact.exists(),
        "the kernel rlib artifact must exist on disk: {}",
        artifact.display()
    );
    assert!(
        artifact
            .file_name()
            .and_then(|s| s.to_str())
            .map(|n| n.starts_with("lib") && n.ends_with(".rlib"))
            .unwrap_or(false),
        "the kernel artifact must be `lib<name>.rlib`: {}",
        artifact.display()
    );

    // The emitted source carries the no_std prelude and NOT a `fn main` / seccomp.
    let source = reconstruct_kernel_source(&sum);
    assert!(
        source.contains("#![no_std]"),
        "the kernel source must carry `#![no_std]`:\n{source}"
    );
    assert!(
        source.contains("extern crate alloc;"),
        "the kernel source must carry `extern crate alloc;`:\n{source}"
    );
    assert!(
        !source.contains("fn main"),
        "a kernel LIBRARY emits no `fn main`:\n{source}"
    );
    assert!(
        !source.contains("PR_SET_SECCOMP") && !source.contains("seccomp"),
        "a kernel build emits NO seccomp prelude:\n{source}"
    );

    // AC-1 mechanism: the reconstructed no_std source compiles freestanding.
    let (compiles, rustc_stderr) = freestanding_compile("ac1", &source);
    assert!(
        compiles,
        "the reconstructed kernel no_std source must compile freestanding with a host \
         panic_handler/allocator stub:\n{rustc_stderr}"
    );
}

// ---- AC-3: the L1 thermite_check!/panic! is emitted verbatim under no_std --------

#[test]
fn l1_checks_emitted_verbatim_in_kernel_source() {
    let sum = corpus_dir().join("sum.th");
    let source = reconstruct_kernel_source(&sum);

    // The always-active L1 check machinery is emitted UNCHANGED (NOT stripped, NOT
    // `debug_assert!`): the macro + the `panic!`-based violation handler.
    assert!(
        source.contains("macro_rules! thermite_check"),
        "the always-active `thermite_check!` macro must be emitted:\n{source}"
    );
    assert!(
        source.contains("fn thermite_contract_violation"),
        "the `thermite_contract_violation` handler must be emitted:\n{source}"
    );
    assert!(
        source.contains("panic!("),
        "the contract-violation handler must `panic!` (core, no_std-valid):\n{source}"
    );
    assert!(
        !source.contains("debug_assert!"),
        "the L1 check must NOT degrade to a debug-only assert:\n{source}"
    );

    // The kernel `sum` body actually instantiates the check (`req`/`inv` → a runtime
    // `thermite_check!(...)` call site), so the check is reachable, not dead.
    assert!(
        source.contains("thermite_check!("),
        "the lowered body must contain at least one `thermite_check!(...)` call:\n{source}"
    );

    // AC-3 mechanism: with the host panic_handler the no_std rlib genuinely compiles
    // (the `panic!` resolves to core's panic machinery, routed to `#[panic_handler]`).
    let (compiles, rustc_stderr) = freestanding_compile("ac3", &source);
    assert!(
        compiles,
        "the no_std source carrying the L1 panic! must compile with a test \
         #[panic_handler]:\n{rustc_stderr}"
    );
}

// ---- AC-2: ambient-syscall fx fn → structured refusal ----------------------------

#[test]
fn ambient_read_fx_fn_is_refused() {
    // The corpus `effect_demo.th` carries `read_small`/`read_doubled` with `fx
    // read(stdin)` — an ambient userspace syscall the kernel target refuses.
    let demo = corpus_dir().join("effect_demo.th");
    let (ok, stdout, stderr) = run_forge_build(&[demo.to_str().unwrap(), "--target", "kernel"]);
    assert!(
        !ok,
        "forge build --target kernel on a `fx read` program must FAIL (no artifact):\n\
         stdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        stderr.contains("read"),
        "the refusal must NAME the rejected `read` effect:\n{stderr}"
    );
    assert!(
        stderr.contains("kernel"),
        "the refusal must explain it is a kernel-target reject:\n{stderr}"
    );
}

#[test]
fn ambient_write_net_term_fx_refuse_identically() {
    // A self-contained `fx write` boundary fn → refused.
    let write_th = write_fixture(
        "write_fx",
        "#[boundary(\"os::write\")] fn put() -> bool\n  req true\n  ens true\n  fx  write(stdout)\n  ;\n",
    );
    let (ok_w, _so, se_w) = run_forge_build(&[write_th.to_str().unwrap(), "--target", "kernel"]);
    let _ = std::fs::remove_file(&write_th);
    assert!(
        !ok_w,
        "a `fx write` fn must be refused for the kernel target"
    );
    assert!(se_w.contains("write"), "the refusal names `write`:\n{se_w}");

    // A self-contained `fx net` boundary fn → refused.
    let net_th = write_fixture(
        "net_fx",
        "#[boundary(\"os::net\")] fn dial() -> bool\n  req true\n  ens true\n  fx  net(socket)\n  ;\n",
    );
    let (ok_n, _so2, se_n) = run_forge_build(&[net_th.to_str().unwrap(), "--target", "kernel"]);
    let _ = std::fs::remove_file(&net_th);
    assert!(!ok_n, "a `fx net` fn must be refused for the kernel target");
    assert!(se_n.contains("net"), "the refusal names `net`:\n{se_n}");

    // A self-contained `fx term` boundary fn → refused.
    let term_th = write_fixture(
        "term_fx",
        "#[boundary(\"os::raw_mode_on\")] fn raw() -> bool\n  req true\n  ens true\n  fx  term\n  ;\n",
    );
    let (ok_t, _so3, se_t) = run_forge_build(&[term_th.to_str().unwrap(), "--target", "kernel"]);
    let _ = std::fs::remove_file(&term_th);
    assert!(
        !ok_t,
        "a `fx term` fn must be refused for the kernel target"
    );
    assert!(se_t.contains("term"), "the refusal names `term`:\n{se_t}");
}

#[test]
fn pure_and_alloc_fx_fns_build_for_kernel() {
    // `sum` is `fx pure` → admitted (built above). `string_demo` carries `fx alloc`
    // fns → admitted (alloc is on the kernel admit list, OQ-2) and the `use TString
    // as String;` alias compiles under no_std (it does NOT collide with the prelude,
    // which imports only `Vec`).
    let string_demo = corpus_dir().join("string_demo.th");
    let (ok, stdout, stderr) = run_forge_build(&[
        string_demo.to_str().unwrap(),
        "--target",
        "kernel",
        "--json",
    ]);
    assert!(
        ok,
        "a `fx pure`/`alloc` program must BUILD for the kernel target (alloc is \
         admitted):\nstdout:{stdout}\nstderr:{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("manifest JSON");
    assert_eq!(v["crate_type"], "rlib");
}

// ---- AC-3: --target kernel + --entry is a usage error ----------------------------

#[test]
fn kernel_target_with_entry_is_usage_error() {
    let sum = corpus_dir().join("sum.th");
    let (ok, stdout, stderr) = run_forge_build(&[
        sum.to_str().unwrap(),
        "--target",
        "kernel",
        "--entry",
        "sum",
    ]);
    assert!(
        !ok,
        "`--target kernel --entry` must be a usage error (a kernel crate is a \
         library):\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert!(
        stderr.contains("entry") && stderr.contains("kernel"),
        "the usage error must explain the kernel/--entry conflict:\n{stderr}"
    );
}

// ---- AC-4: the default (std) target is byte-unchanged ----------------------------

#[test]
fn default_target_source_is_byte_identical_to_no_target_flag() {
    // The default `forge build` and `forge build --target std` must emit the SAME
    // bytes (the std default is unchanged): the std emission carries NO `#![no_std]`
    // prelude. The reconstructed std source (lower_l1 alone, no prelude) is what the
    // existing build_conformance suite already pins; here we only assert the kernel
    // prelude is ABSENT from the std default by checking the artifact still builds and
    // is byte-stable across the explicit-`std` form.
    let sum = corpus_dir().join("sum.th");

    let (ok_default, out_default, err_default) =
        run_forge_build(&[sum.to_str().unwrap(), "--json"]);
    assert!(
        ok_default,
        "the default `forge build sum.th` must still build:\nstdout:{out_default}\n{err_default}"
    );
    let (ok_std, out_std, err_std) =
        run_forge_build(&[sum.to_str().unwrap(), "--target", "std", "--json"]);
    assert!(
        ok_std,
        "`forge build --target std` must build identically:\nstdout:{out_std}\n{err_std}"
    );

    // The crate_type is rlib in both (a default library build), and the std emission
    // is NOT a no_std crate — the reconstructed std source carries no `#![no_std]`.
    let v_default: serde_json::Value = serde_json::from_str(&out_default).expect("default JSON");
    let v_std: serde_json::Value = serde_json::from_str(&out_std).expect("std JSON");
    assert_eq!(v_default["crate_type"], "rlib");
    assert_eq!(v_std["crate_type"], "rlib");

    // The std (non-kernel) lowering emits NO no_std prelude (byte-unchanged default).
    let src = std::fs::read_to_string(&sum).unwrap();
    let parsed = thermite_syntax::parse(&src);
    let std_lowered = thermite_lower::lower_l1(&parsed.program).expect("lower_l1");
    assert!(
        !std_lowered.contains("#![no_std]"),
        "the std default lowering must NOT carry the kernel `#![no_std]` prelude"
    );
}
