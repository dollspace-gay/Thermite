//! Divergence pin (critic audit of #197, commit 14070625): `forge build --target
//! kernel` on a program whose only effect is the DESIGN-ADMITTED `fx time` — the
//! corpus `conformance/effect_link_demo.th` (`#[boundary("os::now")] fn now`,
//! `fx time`) — FAILS with a leaked internal rustc `E0433: cannot find module or
//! crate `std`` instead of building, because `build::emit_source` still emits the
//! Stage-8 `mod os` userspace wrapper (`effect_wrappers::WRAPPERS` `os::now` =
//! `std::time::SystemTime::now()`, a `std::`-qualified body) into the
//! `#![no_std]` kernel crate. `build::reject_ambient_fx_for_kernel` covers only
//! `KERNEL_REJECTED_FX` (`read`/`write`/`net`/`term`), so a `time`-effect
//! boundary SURVIVES the reject with a NON-empty `reachable_boundary_targets`
//! set — falsifying the design's stated invariant.
//!
//! AUTHORITY (`.design/build/kernel-target.md`, the BINDING design):
//!   - OQ-2 resolution: "ONLY `read`/`write`/`net`/`term` are rejected;
//!     `pure`/`alloc`/`panic`/`diverge`/`time`/`rand` are admitted (a kernel
//!     emission carries no syscall mapping at all, so `time`/`rand` are benign
//!     here)." Admitted ⟹ the program BUILDS (REQ-3's reject is the ONLY
//!     refusal class; the implementation's own refusal text says "The admitted
//!     kernel effects are pure/alloc/panic/diverge/time/rand").
//!   - Architecture: `emit_mod_os` "reaches the kernel build with an empty
//!     target set (it emits nothing — no userspace syscall wrapper in a kernel
//!     crate)" — FALSE for an admitted-`fx` (`time`/`rand`) boundary fn.
//!   - REQ-2: the kernel emission is the `#![no_std] + alloc` prelude REUSING
//!     `lower_l1`'s output verbatim — no `std::`-qualified path (OQ-3). The
//!     `mod os { … std::time::SystemTime … }` block violates this profile.
//!   - REQ-3: a kernel refusal is "a NAMED-effect, nonzero-exit, NO-artifact
//!     structured `ForgeError`" — the observed raw rustc-stderr dump (E0433) is
//!     neither a build nor that structured refusal shape.
//!
//! OBSERVED (live, forge at 14070625):
//!   `forge build conformance/effect_link_demo.th --target kernel` → exit 2,
//!   stderr "rustc failed to build the lowered artifact … error[E0433]: cannot
//!   find module or crate `std` … std::time::SystemTime::now()". (The same
//!   E0433 incidentally CONFIRMS the kernel invocation genuinely rejects a
//!   `std::` leak under `#![no_std]` — the no_std claim itself is sound; the
//!   divergence is that forge EMITS such a leak for an admitted effect.)
//!
//! This test asserts the AUTHORITY's behavior (an admitted-`fx` program builds
//! a kernel rlib) and therefore FAILS against the current toolchain. Whether
//! the eventual fix builds the `time` boundary against a no_std wrapper, or
//! amends the design to extend the reject set to BOUNDARY-carrying admitted
//! effects, is the generator's call — under the design as written, the current
//! behavior diverges either way.
//!
//! Tracking: crosslink #198.

use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// `.design/build/kernel-target.md` OQ-2 resolution + REQ-2/REQ-3: `time` is on
/// the kernel ADMIT list, so the `fx time` corpus program
/// `conformance/effect_link_demo.th` must BUILD a kernel rlib (exit 0, an
/// on-disk `lib<name>.rlib` artifact) — not die on a leaked rustc `E0433`
/// because the emitted crate carries the `std::`-bodied `os::now` wrapper
/// inside `#![no_std]`.
#[test]
fn admitted_time_fx_boundary_builds_kernel_rlib() {
    let demo = corpus_dir().join("effect_link_demo.th");
    let out = Command::new(forge_bin())
        .arg("build")
        .arg(&demo)
        .arg("--target")
        .arg("kernel")
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawning `forge build` failed: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // The admit pin: `time` is NOT in `KERNEL_REJECTED_FX`, and the design's
    // OQ-2 resolution calls it benign for the kernel emission — so the build
    // must SUCCEED. (Current behavior: exit 2, "error[E0433]: cannot find
    // module or crate `std`" from the emitted `mod os` `std::time` wrapper.)
    assert!(
        out.status.success(),
        "`forge build --target kernel` on the design-ADMITTED `fx time` corpus \
         program must build a kernel rlib (kernel-target.md OQ-2: time/rand are \
         admitted/benign; REQ-2: the kernel emission carries no `std::` path):\n\
         stdout:{stdout}\nstderr:{stderr}"
    );

    let v: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("manifest JSON: {e}\n{stdout}"));
    assert_eq!(
        v["crate_type"], "rlib",
        "the kernel target produces a library (rlib):\n{stdout}"
    );
    let artifact = PathBuf::from(
        v["artifact"]
            .as_str()
            .unwrap_or_else(|| panic!("no `artifact` field:\n{stdout}")),
    );
    assert!(
        artifact.exists(),
        "the kernel rlib artifact must exist on disk: {}",
        artifact.display()
    );
}
